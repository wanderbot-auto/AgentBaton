use agentbaton_remote_host::protocol::{Binding, Grant};
use agentbaton_remote_host::routing::{
    select_host, Attempt, Candidate, DataLocality, Decision, GrantState, HostObservation, Load,
    NewAttempt, Reachability,
};
use agentbaton_remote_host::transfer::{
    plan_transfer, ApprovalContext, ApprovalRequired, ApprovedScope, Content, ContentError,
    DataItem, DataKind, PlanError, ProjectDataApproval, Sensitivity, TransferRequest,
};

const ABC_SHA256: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
const HELLO_SHA256: &str = "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824";
const EMPTY_SHA256: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
const A: DataItem<'static> = DataItem {
    id: "alpha",
    version: "v1",
    size_bytes: 3,
    sha256: ABC_SHA256,
    sensitivity: Sensitivity::Internal,
    kind: DataKind::ProjectInput,
};
const B: DataItem<'static> = DataItem {
    id: "beta",
    version: "v2",
    size_bytes: 5,
    sha256: HELLO_SHA256,
    sensitivity: Sensitivity::Sensitive,
    kind: DataKind::ProjectInput,
};
const C: DataItem<'static> = DataItem {
    id: "gamma",
    version: "v1",
    size_bytes: 0,
    sha256: EMPTY_SHA256,
    sensitivity: Sensitivity::Public,
    kind: DataKind::ProjectInput,
};

fn binding() -> Binding {
    Binding {
        host: "remote-host".into(),
        project: "project-one".into(),
        connection: "connection-v2".into(),
        attempt: "attempt-one".into(),
        input_revision: "input-revision-2".into(),
        configuration: "config-revision-4".into(),
        identity_ref: "identity-one".into(),
        mapping: "mapping-one".into(),
        recipient: "remote-host".into(),
        sensitivity: "sensitive".into(),
    }
}

fn request<'a>(binding: &'a Binding, necessary_items: &'a [DataItem<'a>]) -> TransferRequest<'a> {
    TransferRequest {
        binding,
        current_input_revision: "input-revision-2",
        necessary_items,
    }
}

fn approve<'a>(
    binding: &'a Binding,
    items: &'a [DataItem<'a>],
    max_total_bytes: u64,
) -> ProjectDataApproval<'a> {
    ProjectDataApproval {
        approval_ref: "transfer-approval-16",
        policy_revision: 4,
        binding,
        scope: Some(ApprovedScope {
            scope_ref: "data-scope-3",
            items,
            max_total_bytes,
        }),
    }
}

#[test]
fn planning_returns_only_the_requested_items_and_exact_approval_context() {
    let binding = binding();
    let approved = [B, C, A];
    let necessary = [C, A];
    let approve = approve(&binding, &approved, 8);
    let plan = plan_transfer(request(&binding, &necessary), Some(approve)).unwrap();
    assert_eq!(plan.binding(), &binding);
    assert_eq!(
        plan.approval(),
        ApprovalContext {
            approval_ref: "transfer-approval-16",
            policy_revision: 4,
            scope_ref: "data-scope-3",
        }
    );
    assert_eq!(plan.entries(), &[A, C]);
    assert_eq!(plan.total_bytes(), 3);
    let reordered = plan_transfer(request(&binding, &[A, C]), Some(approve)).unwrap();
    assert_eq!(plan, reordered);

    let empty = plan_transfer(request(&binding, &[]), Some(approve)).unwrap();
    assert!(empty.entries().is_empty());
    assert_eq!(empty.total_bytes(), 0);
    assert_eq!(empty.verify_content(&[]), Ok(()));
}

#[test]
fn a_selected_binding_requires_separate_project_data_approval() {
    let binding = binding();
    let grant = Grant {
        binding: binding.clone(),
        controller: "controller".into(),
        fence: 1,
        credential_hash: "a".repeat(64),
        issued_ms: 0,
        expires_ms: 100,
        max_operations: 1,
        max_bytes: 100,
        allow_record: true,
    };
    let observation = HostObservation {
        binding: &binding,
        observed_ms: 0,
        valid_until_ms: 100,
        reachability: Reachability::Online,
        capabilities: Some(&[]),
        enforced_isolation: Some(&[]),
        data_locality: Some(DataLocality::OnHost),
        load: Some(Load {
            active_slots: 0,
            total_slots: 1,
        }),
    };
    let attempt = Attempt::New(NewAttempt {
        project: &binding.project,
        attempt: &binding.attempt,
        input_revision: &binding.input_revision,
        sensitivity: &binding.sensitivity,
        approved_bindings: std::slice::from_ref(&binding),
        required_capabilities: &[],
        required_isolation: &[],
    });
    let Decision::Selected { selected, .. } = select_host(
        attempt,
        &[Candidate {
            binding: &binding,
            grant: GrantState::Current(&grant),
            observation: Some(&observation),
        }],
        1,
    ) else {
        panic!("expected an eligible route");
    };
    assert_eq!(
        plan_transfer(request(selected.binding, &[A]), None),
        Err(PlanError::ApprovalRequired(
            ApprovalRequired::MissingProjectApproval
        ))
    );
    let approval = approve(&binding, &[A], 3);
    assert_eq!(
        plan_transfer(
            request(&binding, &[A]),
            Some(ProjectDataApproval {
                scope: None,
                ..approval
            })
        ),
        Err(PlanError::UnknownApprovedScope)
    );
    let known_empty = approve(&binding, &[], 3);
    assert_eq!(
        plan_transfer(request(&binding, &[A]), Some(known_empty)),
        Err(PlanError::ApprovalRequired(
            ApprovalRequired::DataScopeExpansion
        ))
    );
    for invalid in [
        ProjectDataApproval {
            approval_ref: "",
            ..approval
        },
        ProjectDataApproval {
            policy_revision: 0,
            ..approval
        },
    ] {
        assert_eq!(
            plan_transfer(request(&binding, &[A]), Some(invalid)),
            Err(PlanError::InvalidApprovalReference)
        );
    }
}

#[test]
fn a_new_recipient_or_an_added_item_requires_approval_for_the_whole_request() {
    let original = binding();
    let approval = approve(&original, &[A], 20);
    let another_recipient = Binding {
        host: "another-host".into(),
        recipient: "another-host".into(),
        ..original.clone()
    };
    assert_eq!(
        plan_transfer(request(&another_recipient, &[A]), Some(approval)),
        Err(PlanError::ApprovalRequired(ApprovalRequired::NewRecipient))
    );
    // `A` is allowed, but adding `B` rejects the entire plan.
    assert_eq!(
        plan_transfer(request(&original, &[A, B]), Some(approval)),
        Err(PlanError::ApprovalRequired(
            ApprovalRequired::DataScopeExpansion
        ))
    );
    let unrelated = Binding {
        recipient: "another-host".into(),
        ..original.clone()
    };
    assert_eq!(
        plan_transfer(request(&unrelated, &[A]), Some(approval)),
        Err(PlanError::RecipientIsNotHost)
    );
}

#[test]
fn plan_is_bound_to_the_exact_project_attempt_config_identity_mapping_and_input_revision() {
    let original = binding();
    let approval = approve(&original, &[A], 3);
    type BindingChange = fn(&mut Binding);
    let changes: [(BindingChange, PlanError); 8] = [
        (
            |b| b.project = "other-project".into(),
            PlanError::ProjectMismatch,
        ),
        (
            |b| b.connection = "different-connection".into(),
            PlanError::BindingMismatch,
        ),
        (
            |b| b.attempt = "another-attempt".into(),
            PlanError::BindingMismatch,
        ),
        (
            |b| b.input_revision = "input-revision-3".into(),
            PlanError::InputRevisionMismatch,
        ),
        (
            |b| b.configuration = "config-revision-5".into(),
            PlanError::BindingMismatch,
        ),
        (
            |b| b.identity_ref = "different-identity".into(),
            PlanError::BindingMismatch,
        ),
        (
            |b| b.mapping = "different-mapping".into(),
            PlanError::BindingMismatch,
        ),
        (
            |b| b.sensitivity = "internal".into(),
            PlanError::BindingMismatch,
        ),
    ];
    for (change, reason) in changes {
        let mut changed = original.clone();
        change(&mut changed);
        // Keeping the request internally consistent cannot make it match the original approval.
        let changed_request = TransferRequest {
            current_input_revision: &changed.input_revision,
            ..request(&changed, &[A])
        };
        assert_eq!(plan_transfer(changed_request, Some(approval)), Err(reason));
    }
    assert_eq!(
        plan_transfer(
            TransferRequest {
                current_input_revision: "input-revision-3",
                ..request(&original, &[A])
            },
            Some(approval),
        ),
        Err(PlanError::InputRevisionMismatch)
    );
}

#[test]
fn metadata_mismatch_cannot_replace_the_approved_version_content_or_class() {
    let binding = binding();
    let approval = approve(&binding, &[A], 100);
    for (changed, error) in [
        (
            DataItem {
                version: "v-other",
                ..A
            },
            PlanError::ItemVersionMismatch,
        ),
        (DataItem { size_bytes: 4, ..A }, PlanError::ItemSizeMismatch),
        (
            DataItem {
                sha256: HELLO_SHA256,
                ..A
            },
            PlanError::ItemDigestMismatch,
        ),
        (
            DataItem {
                sensitivity: Sensitivity::Public,
                ..A
            },
            PlanError::ItemSensitivityMismatch,
        ),
        (
            DataItem { sha256: "abc", ..A },
            PlanError::InvalidItemDigest,
        ),
        (
            DataItem {
                sha256: &ABC_SHA256.to_ascii_uppercase(),
                ..A
            },
            PlanError::InvalidItemDigest,
        ),
    ] {
        assert_eq!(
            plan_transfer(request(&binding, &[changed]), Some(approval)),
            Err(error)
        );
    }
}

#[test]
fn unknown_sensitivity_and_prohibited_or_unknown_data_classes_fail_closed() {
    let original = binding();
    for (item, error) in [
        (
            DataItem {
                sensitivity: Sensitivity::Unknown,
                ..A
            },
            PlanError::UnknownSensitivity,
        ),
        (
            DataItem {
                kind: DataKind::Secret,
                ..A
            },
            PlanError::ProhibitedDataKind,
        ),
        (
            DataItem {
                kind: DataKind::AccountMaterial,
                ..A
            },
            PlanError::ProhibitedDataKind,
        ),
        (
            DataItem {
                kind: DataKind::Unknown,
                ..A
            },
            PlanError::UnknownDataKind,
        ),
    ] {
        assert_eq!(
            plan_transfer(
                request(&original, &[item]),
                Some(approve(&original, &[A], 3))
            ),
            Err(error)
        );
        let approved = [item, C];
        assert_eq!(
            plan_transfer(
                request(&original, &[C]),
                Some(approve(&original, &approved, 3))
            ),
            Err(error)
        );
    }
    let unknown = Binding {
        sensitivity: "unclassified".into(),
        ..original.clone()
    };
    assert_eq!(
        plan_transfer(request(&unknown, &[A]), Some(approve(&unknown, &[A], 3))),
        Err(PlanError::UnknownSensitivity)
    );
    let lower = Binding {
        sensitivity: "internal".into(),
        ..original
    };
    assert_eq!(
        plan_transfer(request(&lower, &[B]), Some(approve(&lower, &[B], 5))),
        Err(PlanError::SensitivityExceedsBinding)
    );
}

#[test]
fn path_strings_are_not_valid_plan_identifiers() {
    let original = binding();
    for path in [
        "/Users/example/private",
        "../secret",
        "dataset/entry",
        r"C:\Users\example",
    ] {
        let id = DataItem { id: path, ..A };
        let version = DataItem { version: path, ..A };
        assert_eq!(
            plan_transfer(
                request(&original, &[id]),
                Some(approve(&original, &[id], 3))
            ),
            Err(PlanError::InvalidItemId)
        );
        assert_eq!(
            plan_transfer(
                request(&original, &[version]),
                Some(approve(&original, &[version], 3))
            ),
            Err(PlanError::InvalidItemVersion)
        );
        let approval = approve(&original, &[A], 3);
        let scope = ApprovedScope {
            scope_ref: path,
            ..approval.scope.unwrap()
        };
        assert_eq!(
            plan_transfer(
                request(&original, &[A]),
                Some(ProjectDataApproval {
                    scope: Some(scope),
                    ..approval
                })
            ),
            Err(PlanError::InvalidScopeReference)
        );
        let invalid_binding = Binding {
            mapping: path.into(),
            ..original.clone()
        };
        assert_eq!(
            plan_transfer(
                request(&invalid_binding, &[A]),
                Some(approve(&invalid_binding, &[A], 3))
            ),
            Err(PlanError::InvalidBinding)
        );
    }
}

#[test]
fn duplicates_with_equal_or_different_metadata_fail_in_both_manifests() {
    let binding = binding();
    let newer = DataItem { version: "v2", ..A };
    for repeated in [[A, A], [A, newer], [newer, A]] {
        assert_eq!(
            plan_transfer(
                request(&binding, &repeated),
                Some(approve(&binding, &[A], 6))
            ),
            Err(PlanError::DuplicateRequestedId)
        );
        assert_eq!(
            plan_transfer(
                request(&binding, &[A]),
                Some(approve(&binding, &repeated, 6))
            ),
            Err(PlanError::DuplicateApprovedId)
        );
    }
}

#[test]
fn checked_total_enforces_the_exact_byte_ceiling_and_rejects_overflow() {
    let binding = binding();
    let entries = [A, B];
    assert_eq!(
        plan_transfer(
            request(&binding, &entries),
            Some(approve(&binding, &entries, 7))
        ),
        Err(PlanError::ByteBudgetExceeded)
    );
    let exact = plan_transfer(
        request(&binding, &entries),
        Some(approve(&binding, &entries, 8)),
    )
    .unwrap();
    assert_eq!(exact.total_bytes(), 8);
    let huge = DataItem {
        size_bytes: u64::MAX,
        ..A
    };
    let overflowing = [huge, B];
    assert_eq!(
        plan_transfer(
            request(&binding, &overflowing),
            Some(approve(&binding, &overflowing, u64::MAX))
        ),
        Err(PlanError::SizeOverflow)
    );
}

#[test]
fn integrity_verification_accepts_the_complete_set_in_any_order() {
    let binding = binding();
    let entries = [A, B, C];
    let plan = plan_transfer(
        request(&binding, &entries),
        Some(approve(&binding, &entries, 8)),
    )
    .unwrap();
    let content = [
        Content {
            id: C.id,
            version: C.version,
            bytes: b"",
        },
        Content {
            id: B.id,
            version: B.version,
            bytes: b"hello",
        },
        Content {
            id: A.id,
            version: A.version,
            bytes: b"abc",
        },
    ];
    assert_eq!(plan.verify_content(&content), Ok(()));
}

#[test]
fn integrity_verification_rejects_partial_extra_duplicate_stale_or_corrupt_content() {
    let binding = binding();
    let entries = [A, B];
    let plan = plan_transfer(
        request(&binding, &entries),
        Some(approve(&binding, &entries, 8)),
    )
    .unwrap();
    let alpha = Content {
        id: A.id,
        version: A.version,
        bytes: b"abc",
    };
    let beta = Content {
        id: B.id,
        version: B.version,
        bytes: b"hello",
    };
    let extra = Content {
        id: C.id,
        version: C.version,
        bytes: b"",
    };
    assert_eq!(
        plan.verify_content(&[alpha]),
        Err(ContentError::MissingEntry)
    );
    assert_eq!(
        plan.verify_content(&[alpha, beta, extra]),
        Err(ContentError::UnexpectedEntry)
    );
    assert_eq!(
        plan.verify_content(&[alpha, beta, alpha]),
        Err(ContentError::DuplicateId)
    );
    for (changed, error) in [
        (
            Content {
                version: "old",
                ..beta
            },
            ContentError::VersionMismatch,
        ),
        (
            Content {
                bytes: b"hell",
                ..beta
            },
            ContentError::SizeMismatch,
        ),
        (
            Content {
                bytes: b"HELLO",
                ..beta
            },
            ContentError::DigestMismatch,
        ),
    ] {
        assert_eq!(plan.verify_content(&[alpha, changed]), Err(error));
    }
}
