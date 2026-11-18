//! Pure selection over verified project authorization and Host observations.
//!
//! The caller resolves current authority and supplies trusted Unix time in milliseconds.
//! Before dispatch it must revalidate authority, workload availability and the selected binding.

use crate::protocol::{identifier, Binding, Grant};

/// An approved task and the complete eligible binding tuples for that task.
/// A Host's registration, declared metadata, or grant alone cannot populate this scope.
#[derive(Debug, Clone, Copy)]
pub struct NewAttempt<'a> {
    pub project: &'a str,
    pub attempt: &'a str,
    pub input_revision: &'a str,
    pub sensitivity: &'a str,
    pub approved_bindings: &'a [Binding],
    pub required_capabilities: &'a [Capability<'a>],
    pub required_isolation: &'a [IsolationControl],
}

#[derive(Debug, Clone, Copy)]
pub enum Attempt<'a> {
    New(NewAttempt<'a>),
    /// An existing binding with an active, paused, or cancel-requested attempt.
    Active(&'a Binding),
    /// Reconcile this binding before considering another dispatch.
    OutcomeUnknown(&'a Binding),
}

/// The caller resolves live grant state for the verified controller and current fence.
#[derive(Clone, Copy)]
pub enum GrantState<'a> {
    Unknown,
    Revoked,
    Paused,
    /// Already validated by the caller for the intended action, including remaining
    /// authority. Routing rechecks the exact binding and the half-open lease window.
    Current(&'a Grant),
}

#[derive(Clone, Copy)]
pub struct Candidate<'a> {
    pub binding: &'a Binding,
    pub grant: GrantState<'a>,
    /// `None` includes registration-only candidates.
    pub observation: Option<&'a HostObservation<'a>>,
}

/// A caller-verified snapshot. The validity window is bounded by the caller's
/// evidence policy and must cover all of these observations for the exact binding.
#[derive(Debug, Clone, Copy)]
pub struct HostObservation<'a> {
    pub binding: &'a Binding,
    pub observed_ms: i64,
    pub valid_until_ms: i64,
    pub reachability: Reachability,
    /// `Some` lists positively verified capabilities only. `None` is unknown.
    pub capabilities: Option<&'a [Capability<'a>]>,
    /// Actual enforced controls, never requested or declared settings.
    pub enforced_isolation: Option<&'a [IsolationControl]>,
    /// Proven access to the exact input revision through the approved mapping.
    pub data_locality: Option<DataLocality>,
    pub load: Option<Load>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reachability {
    Unknown,
    Offline,
    Online,
}

/// An identifier from the verified capability vocabulary used by the caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Capability<'a>(&'a str);

impl<'a> Capability<'a> {
    pub fn new(name: &'a str) -> Option<Self> {
        identifier(name).then_some(Self(name))
    }

    pub fn as_str(self) -> &'a str {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum IsolationControl {
    /// Workspace separation alone provides no OS sandbox guarantees.
    DedicatedWorkspace,
    FilesystemBoundary,
    NetworkBoundary,
    ProcessBoundary,
}

/// Ordered by preference. Each tier requires evidence of approved data access.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DataLocality {
    OnHost,
    ApprovedSharedReference,
    ApprovedTransfer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Load {
    pub active_slots: u32,
    pub total_slots: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RejectionReason<'a> {
    DuplicateBinding,
    InvalidBinding,
    AttemptBindingMismatch,
    RecipientMismatch,
    SensitivityMismatch,
    OutsideProjectScope,
    GrantUnknown,
    GrantRevoked,
    GrantPaused,
    GrantBindingMismatch,
    InvalidGrantWindow,
    GrantNotYetValid,
    GrantExpired,
    ObservationMissing,
    ObservationBindingMismatch,
    ObservationOutsideValidityWindow,
    ReachabilityUnknown,
    Offline,
    CapabilitiesUnknown,
    MissingCapability(Capability<'a>),
    IsolationUnknown,
    MissingIsolation(IsolationControl),
    DataAccessUnknown,
    LoadUnknown,
    AtCapacity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RejectedCandidate<'a> {
    pub binding: &'a Binding,
    pub reason: RejectionReason<'a>,
}

/// The approved locality and exact utilization inputs behind the ranking.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RankedCandidate<'a> {
    pub binding: &'a Binding,
    pub data_locality: DataLocality,
    pub load: Load,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoundAttemptState {
    Active,
    OutcomeUnknown,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Decision<'a> {
    Selected {
        selected: RankedCandidate<'a>,
        alternatives: Vec<RankedCandidate<'a>>,
        rejected: Vec<RejectedCandidate<'a>>,
    },
    /// Empty rejections mean the caller supplied no candidates.
    Waiting {
        rejected: Vec<RejectedCandidate<'a>>,
    },
    KeepBinding {
        binding: &'a Binding,
        state: BoundAttemptState,
    },
}

/// Filter all candidates before ranking. Eligible candidates are ordered by data
/// locality, then lower utilization. Ties and rejections use the complete binding
/// key, starting with Host ID.
/// Duplicate observations for an exact binding are ambiguous and each is rejected.
pub fn select_host<'a>(
    attempt: Attempt<'a>,
    candidates: &[Candidate<'a>],
    now_ms: i64,
) -> Decision<'a> {
    let scope = match attempt {
        Attempt::New(scope) => scope,
        Attempt::Active(binding) => {
            return Decision::KeepBinding {
                binding,
                state: BoundAttemptState::Active,
            };
        }
        Attempt::OutcomeUnknown(binding) => {
            return Decision::KeepBinding {
                binding,
                state: BoundAttemptState::OutcomeUnknown,
            };
        }
    };

    let mut candidates = candidates.to_vec();
    candidates.sort_unstable_by_key(|candidate| binding_key(candidate.binding));
    let mut eligible = Vec::new();
    let mut rejected = Vec::new();
    for group in candidates.chunk_by(|a, b| a.binding == b.binding) {
        for candidate in group {
            let result = if group.len() == 1 {
                qualify(&scope, candidate, now_ms)
            } else {
                Err(RejectionReason::DuplicateBinding)
            };
            match result {
                Ok(rank) => eligible.push(rank),
                Err(reason) => rejected.push(RejectedCandidate {
                    binding: candidate.binding,
                    reason,
                }),
            }
        }
    }

    eligible.sort_unstable_by(|a, b| {
        a.data_locality
            .cmp(&b.data_locality)
            // Compare utilization fractions exactly. A u32 × u32 product fits in u64.
            .then_with(|| {
                let left = u64::from(a.load.active_slots) * u64::from(b.load.total_slots);
                let right = u64::from(b.load.active_slots) * u64::from(a.load.total_slots);
                left.cmp(&right)
            })
            .then_with(|| binding_key(a.binding).cmp(&binding_key(b.binding)))
    });
    let mut ranked = eligible.into_iter();
    match ranked.next() {
        Some(selected) => Decision::Selected {
            selected,
            alternatives: ranked.collect(),
            rejected,
        },
        None => Decision::Waiting { rejected },
    }
}

fn binding_key(binding: &Binding) -> [&str; 10] {
    let Binding {
        host,
        project,
        connection,
        attempt,
        input_revision,
        configuration,
        identity_ref,
        mapping,
        recipient,
        sensitivity,
    } = binding;
    [
        host,
        project,
        connection,
        attempt,
        input_revision,
        configuration,
        identity_ref,
        mapping,
        recipient,
        sensitivity,
    ]
}

fn check_grant<'a>(
    state: GrantState<'a>,
    binding: &Binding,
    now_ms: i64,
) -> Result<(), RejectionReason<'a>> {
    use RejectionReason::*;

    let grant = match state {
        GrantState::Current(grant) => grant,
        GrantState::Unknown => return Err(GrantUnknown),
        GrantState::Revoked => return Err(GrantRevoked),
        GrantState::Paused => return Err(GrantPaused),
    };
    if &grant.binding != binding {
        return Err(GrantBindingMismatch);
    }
    if grant.issued_ms < 0 || grant.issued_ms >= grant.expires_ms {
        return Err(InvalidGrantWindow);
    }
    if now_ms < grant.issued_ms {
        return Err(GrantNotYetValid);
    }
    if now_ms >= grant.expires_ms {
        return Err(GrantExpired);
    }
    Ok(())
}

fn qualify<'a>(
    scope: &NewAttempt<'a>,
    candidate: &Candidate<'a>,
    now_ms: i64,
) -> Result<RankedCandidate<'a>, RejectionReason<'a>> {
    use RejectionReason::*;

    let binding = candidate.binding;
    if !binding_key(binding).into_iter().all(identifier) {
        return Err(InvalidBinding);
    }
    if binding.project != scope.project
        || binding.attempt != scope.attempt
        || binding.input_revision != scope.input_revision
    {
        return Err(AttemptBindingMismatch);
    }
    if binding.recipient != binding.host {
        return Err(RecipientMismatch);
    }
    if binding.sensitivity != scope.sensitivity {
        return Err(SensitivityMismatch);
    }
    if !scope.approved_bindings.contains(binding) {
        return Err(OutsideProjectScope);
    }
    check_grant(candidate.grant, binding, now_ms)?;

    let observed = candidate.observation.ok_or(ObservationMissing)?;
    if observed.binding != binding {
        return Err(ObservationBindingMismatch);
    }
    if observed.observed_ms < 0
        || now_ms < observed.observed_ms
        || now_ms >= observed.valid_until_ms
    {
        return Err(ObservationOutsideValidityWindow);
    }
    match observed.reachability {
        Reachability::Unknown => return Err(ReachabilityUnknown),
        Reachability::Offline => return Err(Offline),
        Reachability::Online => {}
    }
    let capabilities = observed.capabilities.ok_or(CapabilitiesUnknown)?;
    if let Some(missing) = scope
        .required_capabilities
        .iter()
        .filter(|required| !capabilities.contains(required))
        .min()
    {
        return Err(MissingCapability(*missing));
    }
    let isolation = observed.enforced_isolation.ok_or(IsolationUnknown)?;
    if let Some(missing) = scope
        .required_isolation
        .iter()
        .filter(|required| !isolation.contains(required))
        .min()
    {
        return Err(MissingIsolation(*missing));
    }
    let data_locality = observed.data_locality.ok_or(DataAccessUnknown)?;
    let load = observed.load.ok_or(LoadUnknown)?;
    if load.active_slots >= load.total_slots {
        return Err(AtCapacity);
    }
    Ok(RankedCandidate {
        binding,
        data_locality,
        load,
    })
}
