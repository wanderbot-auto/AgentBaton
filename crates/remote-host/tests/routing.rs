use agentbaton_remote_host::protocol::{Binding, Grant};
use agentbaton_remote_host::routing::{
    select_host, Attempt, BoundAttemptState, Candidate, Capability, DataLocality, Decision,
    GrantState, HostObservation, IsolationControl, Load, NewAttempt, Reachability,
    RejectedCandidate, RejectionReason,
};

const NOW: i64 = 1_000;
const ISOLATION: [IsolationControl; 2] = [
    IsolationControl::DedicatedWorkspace,
    IsolationControl::FilesystemBoundary,
];

struct Host {
    binding: Binding,
    grant: Grant,
    capabilities: [Capability<'static>; 1],
}

impl Host {
    fn new(id: &str) -> Self {
        let binding = Binding {
            host: id.into(),
            project: "project".into(),
            connection: format!("{id}-connection"),
            attempt: "attempt".into(),
            input_revision: "revision".into(),
            configuration: format!("{id}-configuration"),
            identity_ref: format!("{id}-identity"),
            mapping: format!("{id}-mapping"),
            recipient: id.into(),
            sensitivity: "confidential".into(),
        };
        let grant = Grant {
            binding: binding.clone(),
            controller: "controller".into(),
            fence: 1,
            credential_hash: "a".repeat(64),
            issued_ms: NOW - 100,
            expires_ms: NOW + 100,
            max_operations: 1,
            max_bytes: 100,
            allow_record: true,
        };
        Self {
            binding,
            grant,
            capabilities: [Capability::new("record").unwrap()],
        }
    }

    fn observation(&self) -> HostObservation<'_> {
        HostObservation {
            binding: &self.binding,
            observed_ms: NOW - 10,
            valid_until_ms: NOW + 10,
            reachability: Reachability::Online,
            capabilities: Some(&self.capabilities),
            enforced_isolation: Some(&ISOLATION),
            data_locality: Some(DataLocality::OnHost),
            load: Some(Load {
                active_slots: 0,
                total_slots: 2,
            }),
        }
    }

    fn candidate<'a>(&'a self, observation: &'a HostObservation<'a>) -> Candidate<'a> {
        Candidate {
            binding: &self.binding,
            grant: GrantState::Current(&self.grant),
            observation: Some(observation),
        }
    }
}

fn new_attempt<'a>(approved: &'a [Binding], required: &'a [Capability<'a>]) -> Attempt<'a> {
    Attempt::New(NewAttempt {
        project: "project",
        attempt: "attempt",
        input_revision: "revision",
        sensitivity: "confidential",
        approved_bindings: approved,
        required_capabilities: required,
        required_isolation: &ISOLATION,
    })
}

fn assert_rejected<'a>(
    req: Attempt<'a>,
    candidate: Candidate<'a>,
    now: i64,
    reason: RejectionReason<'a>,
) {
    assert_eq!(
        select_host(req, &[candidate], now),
        Decision::Waiting {
            rejected: vec![RejectedCandidate {
                binding: candidate.binding,
                reason,
            }],
        }
    );
}

#[test]
fn selection_returns_the_exact_bound_context_and_scoring_evidence() {
    let host = Host::new("host");
    let observed = host.observation();
    let request = new_attempt(std::slice::from_ref(&host.binding), &host.capabilities);
    let Decision::Selected {
        selected,
        alternatives,
        rejected,
    } = select_host(request, &[host.candidate(&observed)], NOW)
    else {
        panic!("verified candidate should be selected");
    };
    assert_eq!(selected.binding, &host.binding);
    assert_eq!(selected.data_locality, DataLocality::OnHost);
    assert_eq!(selected.load, observed.load.unwrap());
    assert!(alternatives.is_empty() && rejected.is_empty());
    assert_eq!(
        select_host(request, &[], NOW),
        Decision::Waiting { rejected: vec![] }
    );
}

#[test]
fn grant_state_and_half_open_authorization_window_fail_closed() {
    use RejectionReason::*;
    let host = Host::new("host");
    let observed = HostObservation {
        observed_ms: NOW - 200,
        valid_until_ms: NOW + 200,
        ..host.observation()
    };
    let candidate = host.candidate(&observed);
    let request = new_attempt(std::slice::from_ref(&host.binding), &host.capabilities);
    for (grant, reason) in [
        (GrantState::Unknown, GrantUnknown),
        (GrantState::Revoked, GrantRevoked),
        (GrantState::Paused, GrantPaused),
    ] {
        assert_rejected(request, Candidate { grant, ..candidate }, NOW, reason);
    }
    assert_rejected(
        request,
        candidate,
        host.grant.issued_ms - 1,
        GrantNotYetValid,
    );
    assert!(matches!(
        select_host(request, &[candidate], host.grant.issued_ms),
        Decision::Selected { .. }
    ));
    assert_rejected(request, candidate, host.grant.expires_ms, GrantExpired);
    let malformed = Grant {
        expires_ms: host.grant.issued_ms,
        ..host.grant.clone()
    };
    assert_rejected(
        request,
        Candidate {
            grant: GrantState::Current(&malformed),
            ..candidate
        },
        NOW,
        InvalidGrantWindow,
    );
}

#[test]
fn each_binding_dimension_is_covered_by_the_exact_grant_and_observation() {
    let host = Host::new("host");
    let observed = host.observation();
    let candidate = host.candidate(&observed);
    let request = new_attempt(std::slice::from_ref(&host.binding), &host.capabilities);
    let changes: [fn(&mut Binding) -> &mut String; 10] = [
        |binding| &mut binding.host,
        |binding| &mut binding.project,
        |binding| &mut binding.connection,
        |binding| &mut binding.attempt,
        |binding| &mut binding.input_revision,
        |binding| &mut binding.configuration,
        |binding| &mut binding.identity_ref,
        |binding| &mut binding.mapping,
        |binding| &mut binding.recipient,
        |binding| &mut binding.sensitivity,
    ];
    for field in changes {
        let mut other_grant = host.grant.clone();
        field(&mut other_grant.binding).push_str("-other");
        assert_rejected(
            request,
            Candidate {
                grant: GrantState::Current(&other_grant),
                ..candidate
            },
            NOW,
            RejectionReason::GrantBindingMismatch,
        );
        let other_observation = HostObservation {
            binding: &other_grant.binding,
            ..observed
        };
        assert_rejected(
            request,
            host.candidate(&other_observation),
            NOW,
            RejectionReason::ObservationBindingMismatch,
        );
    }
}

#[test]
fn project_scope_uses_complete_bound_tuples_and_the_correct_recipient_and_revision() {
    use RejectionReason::*;
    let host = Host::new("host");
    let other = Host::new("other");
    let approved = [host.binding.clone(), other.binding.clone()];
    type BindingChange = fn(&mut Binding);
    let changes: [(BindingChange, RejectionReason<'_>); 9] = [
        (
            |binding| binding.project = "other".into(),
            AttemptBindingMismatch,
        ),
        (
            |binding| binding.attempt = "other".into(),
            AttemptBindingMismatch,
        ),
        (
            |binding| binding.input_revision = "other".into(),
            AttemptBindingMismatch,
        ),
        (
            |binding| binding.recipient = "other".into(),
            RecipientMismatch,
        ),
        (
            |binding| binding.sensitivity = "public".into(),
            SensitivityMismatch,
        ),
        (
            |binding| binding.mapping = "other-mapping".into(),
            OutsideProjectScope,
        ),
        (
            |binding| binding.identity_ref = "other-identity".into(),
            OutsideProjectScope,
        ),
        (
            |binding| binding.connection = "other-connection".into(),
            OutsideProjectScope,
        ),
        (
            |binding| binding.configuration = "other-configuration".into(),
            OutsideProjectScope,
        ),
    ];
    for (change, reason) in changes {
        let mut switched = Host::new("host");
        change(&mut switched.binding);
        switched.grant.binding = switched.binding.clone();
        let observation = switched.observation();
        assert_rejected(
            new_attempt(&approved, &host.capabilities),
            switched.candidate(&observation),
            NOW,
            reason,
        );
    }

    let mut malformed = Host::new("host");
    malformed.binding.mapping = "/unapproved/absolute/path".into();
    let observation = malformed.observation();
    assert_rejected(
        new_attempt(std::slice::from_ref(&malformed.binding), &host.capabilities),
        malformed.candidate(&observation),
        NOW,
        InvalidBinding,
    );
}

#[test]
fn an_unprobed_registration_or_missing_host_evidence_cannot_route() {
    use RejectionReason::*;
    let host = Host::new("host");
    let observed = host.observation();
    let request = new_attempt(std::slice::from_ref(&host.binding), &host.capabilities);
    assert_rejected(
        request,
        Candidate {
            observation: None,
            ..host.candidate(&observed)
        },
        NOW,
        ObservationMissing,
    );
    let cases = [
        (
            HostObservation {
                reachability: Reachability::Offline,
                ..observed
            },
            Offline,
        ),
        (
            HostObservation {
                reachability: Reachability::Unknown,
                ..observed
            },
            ReachabilityUnknown,
        ),
        (
            HostObservation {
                capabilities: None,
                ..observed
            },
            CapabilitiesUnknown,
        ),
        (
            HostObservation {
                capabilities: Some(&[]),
                ..observed
            },
            MissingCapability(host.capabilities[0]),
        ),
        (
            HostObservation {
                enforced_isolation: None,
                ..observed
            },
            IsolationUnknown,
        ),
        (
            HostObservation {
                enforced_isolation: Some(&[IsolationControl::DedicatedWorkspace]),
                ..observed
            },
            MissingIsolation(IsolationControl::FilesystemBoundary),
        ),
        (
            HostObservation {
                data_locality: None,
                ..observed
            },
            DataAccessUnknown,
        ),
        (
            HostObservation {
                load: None,
                ..observed
            },
            LoadUnknown,
        ),
        (
            HostObservation {
                load: Some(Load {
                    active_slots: 0,
                    total_slots: 0,
                }),
                ..observed
            },
            AtCapacity,
        ),
        (
            HostObservation {
                load: Some(Load {
                    active_slots: 2,
                    total_slots: 2,
                }),
                ..observed
            },
            AtCapacity,
        ),
    ];
    for (observation, reason) in cases {
        assert_rejected(request, host.candidate(&observation), NOW, reason);
    }
    let no_required_features = new_attempt(std::slice::from_ref(&host.binding), &[]);
    let unknown = HostObservation {
        capabilities: None,
        ..observed
    };
    assert_rejected(
        no_required_features,
        host.candidate(&unknown),
        NOW,
        CapabilitiesUnknown,
    );
    let known_empty = HostObservation {
        capabilities: Some(&[]),
        ..observed
    };
    assert!(matches!(
        select_host(no_required_features, &[host.candidate(&known_empty)], NOW),
        Decision::Selected { .. }
    ));
}

#[test]
fn future_stale_or_invalid_observation_windows_cannot_route() {
    let host = Host::new("host");
    let request = new_attempt(std::slice::from_ref(&host.binding), &host.capabilities);
    let observation = host.observation();
    let negative = HostObservation {
        observed_ms: -1,
        ..observation
    };
    for (observed, now) in [
        (&observation, observation.valid_until_ms),
        (&observation, observation.observed_ms - 1),
        (&negative, NOW),
    ] {
        assert_rejected(
            request,
            host.candidate(observed),
            now,
            RejectionReason::ObservationOutsideValidityWindow,
        );
    }
    assert!(matches!(
        select_host(
            request,
            &[host.candidate(&observation)],
            observation.observed_ms
        ),
        Decision::Selected { .. }
    ));
}

#[test]
fn only_eligible_candidates_are_ranked_by_locality_then_exact_utilization() {
    let forbidden = Host::new("a-forbidden");
    let on_host = Host::new("b-on-host");
    let half = Host::new("c-half");
    let third = Host::new("d-third");
    let transfer = Host::new("e-transfer");
    let approved = [&on_host, &half, &third, &transfer].map(|h| h.binding.clone());
    let req = new_attempt(&approved, &on_host.capabilities);
    let forbidden_observed = forbidden.observation();
    let on_host_observed = HostObservation {
        load: Some(Load {
            active_slots: u32::MAX - 1,
            total_slots: u32::MAX,
        }),
        ..on_host.observation()
    };
    let half_observed = HostObservation {
        data_locality: Some(DataLocality::ApprovedSharedReference),
        load: Some(Load {
            active_slots: 1,
            total_slots: 2,
        }),
        ..half.observation()
    };
    let third_observed = HostObservation {
        data_locality: Some(DataLocality::ApprovedSharedReference),
        load: Some(Load {
            active_slots: 2,
            total_slots: 6,
        }),
        ..third.observation()
    };
    let transfer_observed = HostObservation {
        data_locality: Some(DataLocality::ApprovedTransfer),
        ..transfer.observation()
    };
    let candidates = [
        forbidden.candidate(&forbidden_observed),
        half.candidate(&half_observed),
        on_host.candidate(&on_host_observed),
        transfer.candidate(&transfer_observed),
        third.candidate(&third_observed),
    ];
    let Decision::Selected {
        selected,
        alternatives,
        rejected,
    } = select_host(req, &candidates, NOW)
    else {
        panic!("expected an eligible candidate");
    };
    assert_eq!(selected.binding, &on_host.binding);
    assert_eq!(
        alternatives.iter().map(|c| c.binding).collect::<Vec<_>>(),
        vec![&third.binding, &half.binding, &transfer.binding]
    );
    assert_eq!(
        rejected,
        vec![RejectedCandidate {
            binding: &forbidden.binding,
            reason: RejectionReason::OutsideProjectScope,
        }]
    );
}

#[test]
fn equal_scores_and_rejections_are_independent_of_input_order() {
    let alpha = Host::new("alpha");
    let zeta = Host::new("zeta");
    let mut other_connection = Host::new("alpha");
    other_connection.binding.connection = "zz-connection".into();
    other_connection.grant.binding = other_connection.binding.clone();
    let unapproved_a = Host::new("unapproved-a");
    let unapproved_z = Host::new("unapproved-z");
    let approved = [&alpha, &other_connection, &zeta].map(|host| host.binding.clone());
    let req = new_attempt(&approved, &alpha.capabilities);
    let obs = HostObservation {
        load: Some(Load {
            active_slots: u32::MAX - 1,
            total_slots: u32::MAX,
        }),
        ..alpha.observation()
    };
    let other_obs = HostObservation {
        binding: &other_connection.binding,
        ..obs
    };
    let zeta_obs = HostObservation {
        binding: &zeta.binding,
        ..obs
    };
    let unapproved_a_obs = unapproved_a.observation();
    let unapproved_z_obs = unapproved_z.observation();
    let mut candidates = [
        zeta.candidate(&zeta_obs),
        unapproved_z.candidate(&unapproved_z_obs),
        other_connection.candidate(&other_obs),
        unapproved_a.candidate(&unapproved_a_obs),
        alpha.candidate(&obs),
    ];
    let result = select_host(req, &candidates, NOW);
    candidates.reverse();
    assert_eq!(result, select_host(req, &candidates, NOW));
    let Decision::Selected {
        selected,
        alternatives,
        rejected,
    } = result
    else {
        panic!("expected an eligible candidate");
    };
    assert_eq!(selected.binding, &alpha.binding);
    assert_eq!(alternatives[0].binding, &other_connection.binding);
    assert_eq!(alternatives[1].binding, &zeta.binding);
    assert_eq!(rejected[0].binding, &unapproved_a.binding);
    assert_eq!(rejected[1].binding, &unapproved_z.binding);
}

#[test]
fn conflicting_candidates_for_the_same_binding_are_all_rejected() {
    let host = Host::new("host");
    let observed = host.observation();
    let valid = host.candidate(&observed);
    let revoked = Candidate {
        grant: GrantState::Revoked,
        ..valid
    };
    let req = new_attempt(std::slice::from_ref(&host.binding), &host.capabilities);
    let duplicate = RejectedCandidate {
        binding: &host.binding,
        reason: RejectionReason::DuplicateBinding,
    };
    for candidates in [[valid, revoked], [revoked, valid]] {
        assert_eq!(
            select_host(req, &candidates, NOW),
            Decision::Waiting {
                rejected: vec![duplicate, duplicate]
            }
        );
    }
}

#[test]
fn active_and_ambiguous_attempts_keep_the_original_binding_without_reselection() {
    let current = Host::new("current");
    let alternative = Host::new("alternative");
    let observation = alternative.observation();
    let approved = [alternative.binding.clone()];
    let candidate = alternative.candidate(&observation);
    assert!(matches!(
        select_host(
            new_attempt(&approved, &current.capabilities),
            &[candidate],
            NOW
        ),
        Decision::Selected { .. }
    ));
    for (attempt, state) in [
        (Attempt::Active(&current.binding), BoundAttemptState::Active),
        (
            Attempt::OutcomeUnknown(&current.binding),
            BoundAttemptState::OutcomeUnknown,
        ),
    ] {
        assert_eq!(
            select_host(attempt, &[candidate], NOW),
            Decision::KeepBinding {
                binding: &current.binding,
                state
            }
        );
    }
}
