//! Pure product-plan algorithms; synthetic host evidence is not Provider/GUI proof.
use agentbaton_core::{
    execution::ExecutionBinding,
    goal_plan::*,
    policy::{ExecutionGrant, GrantEnforcement, GrantScope, GrantedConnection, NetworkScope},
    storage::{Task, TaskState},
};
use agentbaton_providers::connections::ProtocolRoute;
use agentbaton_workspaces::{WorkspaceIdentity, WorkspaceKind};
use std::collections::{BTreeMap, BTreeSet};
fn map<T>(v: Vec<(&str, T)>) -> BTreeMap<String, T> {
    v.into_iter().map(|(k, v)| (k.into(), v)).collect()
}
fn set(v: &[&str]) -> BTreeSet<String> {
    v.iter().map(|s| (*s).into()).collect()
}
fn sha(c: char) -> String {
    c.to_string().repeat(64)
}
fn budget() -> BudgetLimits {
    BudgetLimits {
        attempts: 20,
        parallel: 4,
        runtime_ms: 10000,
        output_bytes: 10000,
        children: 10,
        reworks: 4,
        tokens: Some(100),
        cost_microunits: Some(100),
    }
}
#[derive(Clone)]
struct Fixture {
    task: Task,
    grant: ExecutionGrant,
    policy: HostGoalPolicy,
}
impl Fixture {
    fn new() -> Self {
        let task = Task {
            id: "task-1".into(),
            project_id: "project-1".into(),
            goal: "Complete approved goal".into(),
            spec_revision: 3,
            state: TaskState::Draft,
            revision: 1,
            acceptance_ref: None,
            acceptance_sequence: None,
            created_at: "host-time".into(),
            updated_at: "host-time".into(),
        };
        let conn = GrantedConnection {
            protocol_route: ProtocolRoute::CodexAppServer,
            revision: 1,
            environment_id: "env-1".into(),
            identity_ref: "identity-1".into(),
            host_id: "host-1".into(),
            config_revision: "config-1".into(),
            executable_sha256: sha('a'),
            capability_snapshot_id: "caps-1".into(),
        };
        let mut workspaces = BTreeMap::new();
        let mut pins = BTreeMap::new();
        for (i, name) in ["workspace-a", "workspace-b", "workspace-c"]
            .iter()
            .enumerate()
        {
            let w = WorkspacePermit {
                host_id: "host-1".into(),
                canonical_root: format!("/fixture/{name}"),
                device: 1,
                inode: i as u64 + 10,
                writable_prefixes: vec!["src".into()],
            };
            pins.insert(
                (*name).into(),
                WorkspaceIdentity {
                    canonical_root: w.canonical_root.clone().into(),
                    device: w.device,
                    inode: w.inode,
                    kind: WorkspaceKind::Directory,
                },
            );
            workspaces.insert((*name).into(), w);
        }
        let grant = ExecutionGrant {
            id: "grant-1".into(),
            project_id: task.project_id.clone(),
            root_grant_id: "grant-1".into(),
            parent_id: None,
            depth: 0,
            revision: 2,
            approval_id: "approval-1".into(),
            approved_at: "host-time".into(),
            expires_at_ms: 100000,
            revoked: false,
            scope: GrantScope {
                workspace_ids: workspaces.keys().cloned().collect(),
                connection_ids: vec!["connection-1".into()],
                host_ids: vec!["host-1".into()],
                max_attempts: 20,
                max_concurrency: 4,
                max_duration_ms: 10000,
                recipients: vec!["reviewer".into()],
                network: NetworkScope::Deny,
                max_depth: 4,
                max_children: 10,
            },
            connection_bindings: map(vec![("connection-1", conn.clone())]),
            workspace_bindings: pins,
            enforcement: GrantEnforcement::default(),
        };
        let policy = HostGoalPolicy {
            root_task_id: task.id.clone(),
            input_revision: "input-1".into(),
            max_nodes: 10,
            max_depth: 4,
            root_budget: budget(),
            project_budget: budget(),
            resources: ResourceLimits {
                global: 4,
                hosts: map(vec![("host-1", 4)]),
                providers: map(vec![("account-quota", 4)]),
                workspaces: map(vec![
                    ("workspace-a", 4),
                    ("workspace-b", 4),
                    ("workspace-c", 4),
                ]),
            },
            connections: map(vec![(
                "connection-1",
                ConnectionPermit {
                    provider_quota_key: "account-quota".into(),
                    binding: conn,
                },
            )]),
            workspaces,
            rules: map(vec![
                (
                    "check",
                    ApprovedRuleRef {
                        kind: RuleKind::Deterministic,
                        policy_digest: sha('b'),
                        goal_acceptance: false,
                    },
                ),
                (
                    "accept",
                    ApprovedRuleRef {
                        kind: RuleKind::Owner,
                        policy_digest: sha('c'),
                        goal_acceptance: true,
                    },
                ),
            ]),
            contracts: map(vec![(
                "result",
                ArtifactContract {
                    required_validators: set(&["check"]),
                },
            )]),
            external_inputs: map(vec![(
                "source",
                ExternalInput {
                    contract_id: "result".into(),
                    sha256: sha('d'),
                    verified: true,
                },
            )]),
            external_actions: BTreeMap::new(),
            external_writes: map(vec![
                (
                    "target-1",
                    ExternalWritePermit {
                        recipient: "reviewer".into(),
                        conflict_group: "same-endpoint".into(),
                    },
                ),
                (
                    "target-alias",
                    ExternalWritePermit {
                        recipient: "reviewer".into(),
                        conflict_group: "same-endpoint".into(),
                    },
                ),
            ]),
            required_goal_outputs: map(vec![("out-a", "result".into())]),
            acceptance_rules: set(&["accept"]),
        };
        let mut fixture = Self {
            task,
            grant,
            policy,
        };
        fixture.approve_action("external-a", "target-1", "workspace-a");
        fixture.approve_action("external-b", "target-alias", "workspace-b");
        fixture
    }
    fn approve_action(&mut self, id: &str, target: &str, workspace: &str) {
        let executor = self.node("template", workspace).executor;
        let request = ExternalActionRequest {
            operation: "create-review".into(),
            recipient: "reviewer".into(),
            resource_ids: set(&[target]),
            payload: serde_json::json!({"body":format!("owner approved exact request {id}")}),
        };
        self.policy.external_actions.insert(
            id.into(),
            ApprovedExternalAction::from_verified_host(executor, request).unwrap(),
        );
    }
    fn scope(&self) -> ApprovedGoalScope {
        ApprovedGoalScope::from_verified_host(&self.task, &self.grant, self.policy.clone(), 10)
            .unwrap()
    }
    fn node(&self, id: &str, workspace: &str) -> PlanNode {
        let c = &self.policy.connections["connection-1"].binding;
        let mut b = budget();
        b.attempts = 3;
        b.parallel = 1;
        b.runtime_ms = 1000;
        b.output_bytes = 1000;
        b.children = 3;
        b.reworks = 2;
        b.tokens = Some(20);
        b.cost_microunits = Some(10);
        PlanNode {
            id: id.into(),
            parent_id: None,
            kind: NodeKind::Work,
            goal: "Produce a checked artifact".into(),
            dependencies: vec![],
            workspace_id: workspace.into(),
            executor: Executor::Connection {
                binding: Box::new(ExecutionBinding {
                    connection_id: "connection-1".into(),
                    environment_id: c.environment_id.clone(),
                    identity_ref: c.identity_ref.clone(),
                    host_id: c.host_id.clone(),
                    workspace_id: workspace.into(),
                    config_revision: c.config_revision.clone(),
                    input_revision: self.policy.input_revision.clone(),
                    capability_snapshot_id: c.capability_snapshot_id.clone(),
                    policy_revision: format!("{}:{}", self.grant.id, self.grant.revision),
                }),
                program_sha256: c.executable_sha256.clone(),
            },
            recipients: vec!["reviewer".into()],
            required_inputs: vec![],
            outputs: vec![PlannedOutput {
                artifact_id: format!("out-{id}"),
                contract_id: "result".into(),
            }],
            required_validators: vec!["check".into()],
            side_effect: SideEffect::AdvisoryReadOnly,
            write_set: vec![],
            budget: b,
            reservation: AttemptReservation {
                runtime_ms: 100,
                output_bytes: 100,
                tokens: Some(5),
                cost_microunits: Some(2),
            },
            retry: RetryPolicy {
                max_attempts: 3,
                max_reworks: 2,
                retry_classes: vec![
                    FailureClass::BeforeDispatchTransient,
                    FailureClass::CrashedNoSideEffect,
                    FailureClass::ValidationFailed,
                ],
            },
            idempotency_key: format!("action-{id}"),
            external_action_id: None,
        }
    }
    fn proposal(&self, nodes: Vec<PlanNode>) -> GoalPlanProposal {
        GoalPlanProposal {
            schema_version: 1,
            plan_id: "plan-1".into(),
            goal: self.scope().goal().clone(),
            scope_revision: self.grant.revision,
            revision: 1,
            expected_revision: 0,
            acceptance_rules: vec!["accept".into()],
            nodes,
        }
    }
    fn plan(&self, nodes: Vec<PlanNode>) -> ValidatedPlan {
        ValidatedPlan::initial(&self.scope(), self.proposal(nodes)).unwrap()
    }
    fn two(&self) -> ValidatedPlan {
        self.plan(vec![
            self.node("a", "workspace-a"),
            self.node("b", "workspace-b"),
        ])
    }
    fn isolate(&self, p: &ValidatedPlan, h: &mut HostPlanSnapshot) {
        h.verified_isolation.push(IsolationProof {
            evidence: p.evidence_binding(),
            left_workspace: "workspace-a".into(),
            right_workspace: "workspace-b".into(),
            left: self.policy.workspaces["workspace-a"].clone(),
            right: self.policy.workspaces["workspace-b"].clone(),
        });
        h.verified_write_sets = p
            .proposal()
            .nodes
            .iter()
            .map(|n| WriteSetProof {
                evidence: p.evidence_binding(),
                node_id: n.id.clone(),
                workspace_id: n.workspace_id.clone(),
                write_set: n.write_set.clone(),
            })
            .collect();
    }
}
fn snapshot(p: &ValidatedPlan) -> HostPlanSnapshot {
    HostPlanSnapshot {
        goal: p.proposal().goal.clone(),
        plan_digest: p.digest().into(),
        scope_revision: p.proposal().scope_revision,
        ledger_revision: 8,
        now_ms: 20,
        revoked: false,
        cancelled: false,
        nodes: p
            .proposal()
            .nodes
            .iter()
            .map(|n| {
                (
                    n.id.clone(),
                    NodeObservation {
                        evidence: p.evidence_binding(),
                        state: NodeState::NotStarted,
                        attempt_id: None,
                        attempts_started: 0,
                        reworks_used: 0,
                        consumed_inputs: vec![],
                    },
                )
            })
            .collect(),
        artifacts: vec![],
        validations: vec![],
        root_usage: BudgetUsage {
            tokens: Some(0),
            cost_microunits: Some(0),
            ..Default::default()
        },
        project_usage: BudgetUsage {
            tokens: Some(0),
            cost_microunits: Some(0),
            ..Default::default()
        },
        subtree_usage: p
            .proposal()
            .nodes
            .iter()
            .map(|n| {
                (
                    n.id.clone(),
                    BudgetUsage {
                        tokens: Some(0),
                        cost_microunits: Some(0),
                        ..Default::default()
                    },
                )
            })
            .collect(),
        resources: ResourceUsage::default(),
        unresolved_actions: vec![],
        external_actions: p
            .approved_external_actions()
            .iter()
            .map(|(id, a)| {
                (
                    id.clone(),
                    ExternalActionObservation {
                        project_id: p.proposal().goal.project_id.clone(),
                        root_task_id: p.proposal().goal.root_task_id.clone(),
                        action_id: id.clone(),
                        payload_sha256: a.payload_sha256.clone(),
                        state: ExternalActionState::NeverStarted,
                    },
                )
            })
            .collect(),
        verified_isolation: vec![],
        verified_write_sets: vec![],
    }
}
fn observe(p: &ValidatedPlan, h: &mut HostPlanSnapshot, node: &str, state: NodeState) {
    let inputs = p
        .proposal()
        .nodes
        .iter()
        .find(|n| n.id == node)
        .unwrap()
        .required_inputs
        .iter()
        .map(|i| ConsumedInputReceipt {
            evidence: p.evidence_binding(),
            consumer_node_id: node.into(),
            consumer_attempt_id: format!("attempt-{node}"),
            input: resolve_for_test(p, h, i),
        })
        .collect();
    let o = h.nodes.get_mut(node).unwrap();
    o.consumed_inputs = inputs;
    o.state = state;
    o.attempt_id = Some(format!("attempt-{node}"));
    o.attempts_started = 1;
    h.root_usage.attempts_started = h.nodes.values().map(|o| o.attempts_started).sum();
    h.root_usage.runtime_ms_reserved = u64::from(h.root_usage.attempts_started) * 100;
    h.root_usage.output_bytes_reserved = u64::from(h.root_usage.attempts_started) * 100;
    h.resources = ResourceUsage::default();
    for n in &p.proposal().nodes {
        if matches!(
            h.nodes[&n.id].state,
            NodeState::Running
                | NodeState::CancelRequested
                | NodeState::OutcomeUnknown
                | NodeState::Failed {
                    stop: StopProof::Unknown,
                    ..
                }
        ) {
            h.resources.global += 1;
            *h.resources.hosts.entry("host-1".into()).or_default() += 1;
            *h.resources
                .workspaces
                .entry(n.workspace_id.clone())
                .or_default() += 1;
            if matches!(n.executor, Executor::Connection { .. }) {
                *h.resources
                    .providers
                    .entry("account-quota".into())
                    .or_default() += 1;
            }
        }
    }
    h.root_usage.active_slots = h.resources.global;
    h.root_usage.children_started = p
        .proposal()
        .nodes
        .iter()
        .filter(|n| n.parent_id.is_some() && h.nodes[&n.id].attempts_started > 0)
        .count() as u32;
    h.root_usage.tokens = Some(u64::from(h.root_usage.attempts_started) * 5);
    h.root_usage.cost_microunits = Some(u64::from(h.root_usage.attempts_started) * 2);
    for n in &p.proposal().nodes {
        let mut u = BudgetUsage {
            tokens: Some(0),
            cost_microunits: Some(0),
            ..Default::default()
        };
        for child in &p.proposal().nodes {
            let mut cur = child;
            let belongs = loop {
                if cur.id == n.id {
                    break true;
                }
                match &cur.parent_id {
                    Some(id) => cur = p.proposal().nodes.iter().find(|n| &n.id == id).unwrap(),
                    None => break false,
                }
            };
            if belongs {
                let o = &h.nodes[&child.id];
                u.attempts_started += o.attempts_started;
                u.reworks_used += o.reworks_used;
                if child.id != n.id && o.attempts_started > 0 {
                    u.children_started += 1;
                }
                if matches!(
                    o.state,
                    NodeState::Running
                        | NodeState::CancelRequested
                        | NodeState::OutcomeUnknown
                        | NodeState::Failed {
                            stop: StopProof::Unknown,
                            ..
                        }
                ) {
                    u.active_slots += 1;
                }
            }
        }
        u.runtime_ms_reserved = u64::from(u.attempts_started) * 100;
        u.output_bytes_reserved = u64::from(u.attempts_started) * 100;
        u.tokens = Some(u64::from(u.attempts_started) * 5);
        u.cost_microunits = Some(u64::from(u.attempts_started) * 2);
        h.subtree_usage.insert(n.id.clone(), u);
    }
    h.project_usage = h.root_usage.clone();
}
fn artifact(p: &ValidatedPlan, h: &mut HostPlanSnapshot, node: &str) {
    h.artifacts.push(ArtifactReceipt {
        evidence: p.evidence_binding(),
        node_id: node.into(),
        attempt_id: format!("attempt-{node}"),
        artifact_id: format!("out-{node}"),
        sha256: sha('e'),
    });
}
fn receipt(p: &ValidatedPlan, h: &mut HostPlanSnapshot, node: &str, rule: &str, passed: bool) {
    let a = h.artifacts.iter().find(|a| a.node_id == node).unwrap();
    h.validations.push(ValidationReceipt {
        evidence: p.evidence_binding(),
        input: ResolvedInput {
            artifact_id: a.artifact_id.clone(),
            contract_id: p
                .proposal()
                .nodes
                .iter()
                .find(|n| n.id == node)
                .unwrap()
                .outputs
                .iter()
                .find(|o| o.artifact_id == a.artifact_id)
                .unwrap()
                .contract_id
                .clone(),
            source: ResolvedInputSource::NodeOutput {
                node_id: node.into(),
                attempt_id: a.attempt_id.clone(),
                sha256: a.sha256.clone(),
            },
        },
        rule_id: rule.into(),
        policy_digest: sha(if rule == "check" { 'b' } else { 'c' }),
        passed,
    });
}
fn resolve_for_test(p: &ValidatedPlan, h: &HostPlanSnapshot, i: &RequiredInput) -> ResolvedInput {
    let source = match &i.source {
        InputSource::ApprovedExternal { sha256 } => ResolvedInputSource::ApprovedExternal {
            scope_revision: p.proposal().scope_revision,
            sha256: sha256.clone(),
        },
        InputSource::Node { node_id, .. } => {
            let a = h
                .artifacts
                .iter()
                .find(|a| a.artifact_id == i.artifact_id)
                .expect("fixture input must exist");
            ResolvedInputSource::NodeOutput {
                node_id: node_id.clone(),
                attempt_id: a.attempt_id.clone(),
                sha256: a.sha256.clone(),
            }
        }
    };
    ResolvedInput {
        artifact_id: i.artifact_id.clone(),
        contract_id: i.contract_id.clone(),
        source,
    }
}
fn decision(p: &ValidatedPlan, h: HostPlanSnapshot) -> ReadyDecision {
    decide_ready(p, &TrustedPlanSnapshot::from_verified_host(p, h).unwrap()).unwrap()
}
fn input(from: &str, candidate: bool) -> RequiredInput {
    RequiredInput {
        goal_task_id: "task-1".into(),
        artifact_id: format!("out-{from}"),
        contract_id: "result".into(),
        source: InputSource::Node {
            node_id: from.into(),
            candidate_for_validation: candidate,
        },
    }
}
#[test]
fn strict_agent_dto_cannot_submit_authority_or_passed_evidence() {
    let f = Fixture::new();
    let p = f.proposal(vec![f.node("a", "workspace-a")]);
    for field in ["passed", "actor", "approval", "scope", "status"] {
        let mut v = serde_json::to_value(&p).unwrap();
        v[field] = true.into();
        assert!(serde_json::from_value::<GoalPlanProposal>(v).is_err());
    }
    let mut v = serde_json::to_value(p).unwrap();
    v["nodes"][0]["verified_isolation"] = true.into();
    assert!(serde_json::from_value::<GoalPlanProposal>(v).is_err());
}
#[test]
fn version_cas_and_spec_input_binding_are_exact() {
    let f = Fixture::new();
    let p = f.plan(vec![f.node("a", "workspace-a")]);
    let mut next = p.proposal().clone();
    next.revision = 2;
    next.expected_revision = 1;
    let revised = p.revise(&f.scope(), next.clone()).unwrap();
    assert_ne!(p.digest(), revised.digest());
    assert_eq!(
        revised.revise(&f.scope(), next).unwrap_err(),
        PlanError::StaleRevision
    );
    for alter in 0..3 {
        let mut bad = p.proposal().clone();
        match alter {
            0 => bad.goal.spec_revision += 1,
            1 => bad.goal.input_revision = "other".into(),
            _ => bad.goal.task_id = "other".into(),
        };
        assert_eq!(
            ValidatedPlan::initial(&f.scope(), bad).unwrap_err(),
            PlanError::InputChanged
        );
    }
    let h = TrustedPlanSnapshot::from_verified_host(&p, snapshot(&p)).unwrap();
    assert_eq!(
        decide_ready(&revised, &h).unwrap_err(),
        PlanError::StaleEvidence
    );
}
#[test]
fn changed_scope_and_acceptance_cannot_be_self_approved() {
    let f = Fixture::new();
    let p = f.plan(vec![f.node("a", "workspace-a")]);
    let mut q = p.proposal().clone();
    q.expected_revision = 1;
    q.revision = 2;
    q.acceptance_rules = vec!["check".into()];
    assert_eq!(
        p.revise(&f.scope(), q).unwrap_err(),
        PlanError::AuthorityExpansion
    );
    let mut changed = f.clone();
    changed.grant.expires_at_ms -= 1;
    let mut q = p.proposal().clone();
    q.expected_revision = 1;
    q.revision = 2;
    assert_eq!(
        p.revise(&changed.scope(), q).unwrap_err(),
        PlanError::StaleRevision
    );
}
#[test]
fn actual_root_authority_cannot_be_expanded_by_proposal() {
    let f = Fixture::new();
    for alter in 0..9 {
        let mut n = f.node("a", "workspace-a");
        match alter {
            0 => n.side_effect = SideEffect::Production,
            1 => n.recipients.push("stranger".into()),
            2 => n.budget.attempts = 21,
            3 => n.workspace_id = "new-workspace".into(),
            _ => {
                if let Executor::Connection {
                    binding,
                    program_sha256,
                } = &mut n.executor
                {
                    match alter {
                        4 => binding.identity_ref = "other-account".into(),
                        5 => binding.config_revision = "other".into(),
                        6 => binding.host_id = "other".into(),
                        7 => binding.environment_id = "other".into(),
                        _ => *program_sha256 = sha('f'),
                    }
                }
            }
        };
        assert_eq!(
            ValidatedPlan::initial(&f.scope(), f.proposal(vec![n])).unwrap_err(),
            PlanError::AuthorityExpansion
        );
    }
}
#[test]
fn host_constructor_validates_narrowing_but_does_not_authenticate_labels() {
    let mut f = Fixture::new();
    f.grant.revoked = true;
    assert_eq!(
        ApprovedGoalScope::from_verified_host(&f.task, &f.grant, f.policy.clone(), 10).unwrap_err(),
        PlanError::AuthorityExpansion
    );
    f.grant.revoked = false;
    f.policy.root_budget.parallel = 5;
    assert!(
        ApprovedGoalScope::from_verified_host(&f.task, &f.grant, f.policy.clone(), 10).is_err()
    );
    f.policy.root_budget.parallel = 4;
    f.policy
        .connections
        .get_mut("connection-1")
        .unwrap()
        .binding
        .identity_ref = "unapproved".into();
    assert!(
        ApprovedGoalScope::from_verified_host(&f.task, &f.grant, f.policy.clone(), 10).is_err()
    );
}
#[test]
fn missing_refs_cycles_duplicate_ids_outputs_and_keys_reject() {
    let f = Fixture::new();
    for alter in 0..6 {
        let mut a = f.node("a", "workspace-a");
        let mut b = f.node("b", "workspace-b");
        match alter {
            0 => a.dependencies.push("missing".into()),
            1 => {
                a.dependencies.push("b".into());
                b.dependencies.push("a".into());
            }
            2 => b.id = a.id.clone(),
            3 => b.outputs = a.outputs.clone(),
            4 => b.idempotency_key = a.idempotency_key.clone(),
            _ => {
                a.parent_id = Some("b".into());
                b.parent_id = Some("a".into());
            }
        }
        assert!(ValidatedPlan::initial(&f.scope(), f.proposal(vec![a, b])).is_err());
    }
}
#[test]
fn parent_lineage_only_narrows_child_budget_and_recipients() {
    let f = Fixture::new();
    let mut a = f.node("a", "workspace-a");
    let mut b = f.node("b", "workspace-b");
    b.parent_id = Some("a".into());
    a.budget.attempts = 2;
    a.retry.max_attempts = 2;
    assert_eq!(
        ValidatedPlan::initial(&f.scope(), f.proposal(vec![a, b])).unwrap_err(),
        PlanError::AuthorityExpansion
    );
}
#[test]
fn external_inputs_cross_goal_drift_and_missing_validators_reject() {
    let f = Fixture::new();
    let mut n = f.node("a", "workspace-a");
    n.required_inputs.push(RequiredInput {
        goal_task_id: "task-1".into(),
        artifact_id: "source".into(),
        contract_id: "result".into(),
        source: InputSource::ApprovedExternal { sha256: sha('d') },
    });
    f.plan(vec![n.clone()]);
    n.required_inputs[0].goal_task_id = "other-goal".into();
    assert_eq!(
        ValidatedPlan::initial(&f.scope(), f.proposal(vec![n.clone()])).unwrap_err(),
        PlanError::InputChanged
    );
    n.required_inputs[0].goal_task_id = "task-1".into();
    n.required_inputs[0].source = InputSource::ApprovedExternal { sha256: sha('e') };
    assert_eq!(
        ValidatedPlan::initial(&f.scope(), f.proposal(vec![n])).unwrap_err(),
        PlanError::InputChanged
    );
    let mut n = f.node("a", "workspace-a");
    n.required_validators.clear();
    assert_eq!(
        ValidatedPlan::initial(&f.scope(), f.proposal(vec![n])).unwrap_err(),
        PlanError::MissingReference
    );
}
#[test]
fn execution_complete_is_not_validation_or_goal_acceptance() {
    let f = Fixture::new();
    let p = f.plan(vec![f.node("a", "workspace-a")]);
    let mut h = snapshot(&p);
    observe(&p, &mut h, "a", NodeState::ExecutionSucceeded);
    artifact(&p, &mut h, "a");
    assert!(!decision(&p, h.clone()).required_checks_satisfied);
    receipt(&p, &mut h, "a", "check", true);
    assert!(!decision(&p, h.clone()).required_checks_satisfied);
    receipt(&p, &mut h, "a", "accept", true);
    assert!(decision(&p, h.clone()).required_checks_satisfied);
    h.validations[0].passed = false;
    assert!(!decision(&p, h).required_checks_satisfied);
}
#[test]
fn stale_wrong_attempt_rule_digest_and_artifact_hash_are_rejected() {
    let f = Fixture::new();
    let p = f.plan(vec![f.node("a", "workspace-a")]);
    let mut h = snapshot(&p);
    observe(&p, &mut h, "a", NodeState::ExecutionSucceeded);
    artifact(&p, &mut h, "a");
    receipt(&p, &mut h, "a", "check", true);
    for alter in 0..5 {
        let mut b = h.clone();
        match alter {
            0 => {
                if let ResolvedInputSource::NodeOutput { attempt_id, .. } =
                    &mut b.validations[0].input.source
                {
                    *attempt_id = "old-attempt".into()
                }
            }
            1 => {
                if let ResolvedInputSource::NodeOutput { sha256, .. } =
                    &mut b.validations[0].input.source
                {
                    *sha256 = sha('f')
                }
            }
            2 => b.validations[0].policy_digest = sha('f'),
            3 => b.validations[0].evidence.plan_revision += 1,
            _ => b.nodes.get_mut("a").unwrap().evidence.goal.spec_revision += 1,
        };
        assert_eq!(
            TrustedPlanSnapshot::from_verified_host(&p, b).unwrap_err(),
            PlanError::StaleEvidence
        );
    }
}
#[test]
fn ordinary_downstream_requires_exact_passed_inputs() {
    let f = Fixture::new();
    let a = f.node("a", "workspace-a");
    let mut b = f.node("b", "workspace-b");
    b.dependencies.push("a".into());
    b.required_inputs.push(input("a", false));
    let p = f.plan(vec![a, b]);
    assert_eq!(p.topological_order(), &["a", "b"]);
    let mut h = snapshot(&p);
    observe(&p, &mut h, "a", NodeState::ExecutionSucceeded);
    artifact(&p, &mut h, "a");
    assert_eq!(
        decision(&p, h.clone()).blocked["b"],
        BlockReason::Dependencies
    );
    receipt(&p, &mut h, "a", "check", true);
    assert_eq!(decision(&p, h).admissions[0].node_id, "b");
}
#[test]
fn real_validator_can_consume_candidate_without_self_validation_deadlock() {
    let f = Fixture::new();
    let a = f.node("a", "workspace-a");
    let mut v = f.node("validator", "workspace-b");
    v.kind = NodeKind::Validate;
    v.executor = Executor::Validator {
        rule_id: "check".into(),
    };
    v.outputs.clear();
    v.dependencies.push("a".into());
    v.required_inputs.push(input("a", true));
    let p = f.plan(vec![a, v]);
    let mut h = snapshot(&p);
    observe(&p, &mut h, "a", NodeState::ExecutionSucceeded);
    artifact(&p, &mut h, "a");
    let d = decision(&p, h.clone());
    assert_eq!(d.admissions[0].node_id, "validator");
    observe(&p, &mut h, "validator", NodeState::ExecutionSucceeded);
    receipt(&p, &mut h, "a", "accept", true);
    assert!(!decision(&p, h.clone()).required_checks_satisfied);
    receipt(&p, &mut h, "a", "check", true);
    assert!(decision(&p, h).required_checks_satisfied);
}
#[test]
fn agent_work_cannot_consume_unvalidated_candidates_or_impersonate_validator() {
    let f = Fixture::new();
    let a = f.node("a", "workspace-a");
    let mut b = f.node("b", "workspace-b");
    b.required_inputs.push(input("a", true));
    assert_eq!(
        ValidatedPlan::initial(&f.scope(), f.proposal(vec![a, b])).unwrap_err(),
        PlanError::AuthorityExpansion
    );
    let mut a = f.node("a", "workspace-a");
    a.kind = NodeKind::Validate;
    assert_eq!(
        ValidatedPlan::initial(&f.scope(), f.proposal(vec![a])).unwrap_err(),
        PlanError::AuthorityExpansion
    );
}
#[test]
fn readonly_labels_do_not_prove_safe_parallelism() {
    let f = Fixture::new();
    let p = f.two();
    let h = snapshot(&p);
    let d = decision(&p, h);
    assert_eq!(d.admissions.len(), 1);
    assert_eq!(d.blocked["b"], BlockReason::WriteConflict);
    let p = f.plan(vec![f.node("a", "workspace-a"), f.node("b", "workspace-a")]);
    let mut h = snapshot(&p);
    f.isolate(&p, &mut h);
    assert_eq!(decision(&p, h).admissions.len(), 1);
}
#[test]
fn exact_host_isolation_and_write_proofs_allow_distinct_workspace_parallelism() {
    let f = Fixture::new();
    let p = f.two();
    let mut h = snapshot(&p);
    f.isolate(&p, &mut h);
    assert_eq!(decision(&p, h.clone()).admissions.len(), 2);
    h.verified_write_sets[0].workspace_id = "workspace-b".into();
    assert_eq!(
        TrustedPlanSnapshot::from_verified_host(&p, h).unwrap_err(),
        PlanError::StaleEvidence
    );
}
#[test]
fn alias_case_and_unknown_unicode_paths_remain_conservative() {
    for root in [
        "/fixture/WORKSPACE-A",
        "/fixture/workspace-a/sub",
        "/fixture/中文",
    ] {
        let mut f = Fixture::new();
        f.policy
            .workspaces
            .get_mut("workspace-b")
            .unwrap()
            .canonical_root = root.into();
        f.grant
            .workspace_bindings
            .get_mut("workspace-b")
            .unwrap()
            .canonical_root = root.into();
        let p = f.two();
        let mut h = snapshot(&p);
        f.isolate(&p, &mut h);
        assert_eq!(decision(&p, h).admissions.len(), 1);
    }
}
#[test]
fn external_resource_aliases_conflict_even_in_isolated_workspaces() {
    let f = Fixture::new();
    let mut a = f.node("a", "workspace-a");
    let mut b = f.node("b", "workspace-b");
    for (n, target) in [(&mut a, "target-1"), (&mut b, "target-alias")] {
        n.side_effect = SideEffect::ExternalWrite;
        n.external_action_id = Some(
            if target == "target-1" {
                "external-a"
            } else {
                "external-b"
            }
            .into(),
        );
        n.write_set = vec![WriteTarget::External {
            resource_id: target.into(),
        }];
    }
    let p = f.plan(vec![a, b]);
    let mut h = snapshot(&p);
    f.isolate(&p, &mut h);
    assert_eq!(decision(&p, h).admissions.len(), 1);
}
#[test]
fn path_authority_rejects_traversal_and_unapproved_prefixes() {
    let f = Fixture::new();
    for path in [
        "../src/x", "/src/x", "src/../x", "src\\x", "other/x", "src//x",
    ] {
        let mut a = f.node("a", "workspace-a");
        a.side_effect = SideEffect::WorkspaceWrite;
        a.write_set = vec![WriteTarget::Workspace { path: path.into() }];
        assert_eq!(
            ValidatedPlan::initial(&f.scope(), f.proposal(vec![a])).unwrap_err(),
            PlanError::AuthorityExpansion
        );
    }
}
#[test]
fn all_shared_resource_dimensions_bound_parallel_batches() {
    for dimension in 0..4 {
        let mut f = Fixture::new();
        match dimension {
            0 => f.policy.resources.global = 1,
            1 => *f.policy.resources.hosts.get_mut("host-1").unwrap() = 1,
            2 => {
                *f.policy
                    .resources
                    .providers
                    .get_mut("account-quota")
                    .unwrap() = 1
            }
            _ => {
                *f.policy
                    .resources
                    .workspaces
                    .get_mut("workspace-b")
                    .unwrap() = 1
            }
        };
        let p = f.two();
        let mut h = snapshot(&p);
        f.isolate(&p, &mut h);
        if dimension == 3 {
            h.resources.workspaces.insert("workspace-b".into(), 1);
        }
        let d = decision(&p, h);
        assert_eq!(d.admissions.len(), 1);
        assert_eq!(d.blocked["b"], BlockReason::ResourceLimit);
    }
}
#[test]
fn root_and_project_ledgers_include_history_and_sibling_reservations() {
    let f = Fixture::new();
    let p = f.two();
    let mut h = snapshot(&p);
    f.isolate(&p, &mut h);
    h.root_usage.attempts_started = 19;
    h.project_usage = h.root_usage.clone();
    let d = decision(&p, h);
    assert_eq!(d.admissions.len(), 1);
    assert_eq!(d.blocked["b"], BlockReason::RootBudget);
    let mut h = snapshot(&p);
    h.project_usage.attempts_started = 20;
    let d = decision(&p, h);
    assert!(d.admissions.is_empty());
    assert_eq!(d.blocked["a"], BlockReason::ProjectBudget);
}
#[test]
fn unknown_cost_is_not_zero_and_budget_arithmetic_never_wraps() {
    let f = Fixture::new();
    let p = f.two();
    let mut h = snapshot(&p);
    h.root_usage.tokens = None;
    assert_eq!(decision(&p, h).blocked["a"], BlockReason::MeteringUnknown);
    let mut h = snapshot(&p);
    h.root_usage.output_bytes_reserved = u64::MAX;
    h.project_usage = h.root_usage.clone();
    assert_eq!(decision(&p, h).blocked["a"], BlockReason::RootBudget);
    let mut h = snapshot(&p);
    h.root_usage.tokens = Some(u64::MAX);
    h.project_usage.tokens = Some(u64::MAX);
    assert_eq!(decision(&p, h).blocked["a"], BlockReason::RootBudget);
}
#[test]
fn underreported_active_resources_and_attempt_ledgers_reject() {
    let f = Fixture::new();
    let p = f.two();
    let mut h = snapshot(&p);
    observe(&p, &mut h, "a", NodeState::Running);
    h.resources.providers.clear();
    assert_eq!(
        TrustedPlanSnapshot::from_verified_host(&p, h).unwrap_err(),
        PlanError::StaleEvidence
    );
    let mut h = snapshot(&p);
    observe(&p, &mut h, "a", NodeState::ExecutionSucceeded);
    h.root_usage.attempts_started = 0;
    assert_eq!(
        TrustedPlanSnapshot::from_verified_host(&p, h).unwrap_err(),
        PlanError::StaleEvidence
    );
}
#[test]
fn unknown_outcomes_block_new_keys_new_nodes_and_old_plan_actions() {
    let f = Fixture::new();
    let p = f.two();
    let mut h = snapshot(&p);
    observe(&p, &mut h, "a", NodeState::OutcomeUnknown);
    let d = decision(&p, h);
    assert!(d.admissions.is_empty());
    assert_eq!(d.blocked["b"], BlockReason::ReconcileUnknown);
    let mut h = snapshot(&p);
    h.unresolved_actions.push(UnresolvedAction {
        root_task_id: "task-1".into(),
        action_id: "old-plan-action".into(),
        attempt_id: "old-attempt".into(),
    });
    assert!(decision(&p, h).admissions.is_empty());
}
#[test]
fn retry_and_rework_require_stop_evidence_safe_class_and_root_caps() {
    let f = Fixture::new();
    let p = f.plan(vec![f.node("a", "workspace-a")]);
    for (class, stop, expected) in [
        (
            FailureClass::BeforeDispatchTransient,
            StopProof::ConfirmedNotStarted,
            None,
        ),
        (
            FailureClass::Authentication,
            StopProof::ConfirmedStopped,
            Some(BlockReason::RetryNotAllowed),
        ),
        (
            FailureClass::CrashedNoSideEffect,
            StopProof::Unknown,
            Some(BlockReason::ReconcileUnknown),
        ),
    ] {
        let mut h = snapshot(&p);
        observe(&p, &mut h, "a", NodeState::Failed { class, stop });
        let d = decision(&p, h);
        if let Some(r) = expected {
            assert_eq!(d.blocked["a"], r)
        } else {
            assert_eq!(d.admissions[0].next_attempt, 2);
            assert_eq!(d.admissions[0].idempotency_key.len(), 64)
        }
    }
    let mut h = snapshot(&p);
    observe(
        &p,
        &mut h,
        "a",
        NodeState::Failed {
            class: FailureClass::ValidationFailed,
            stop: StopProof::ConfirmedStopped,
        },
    );
    h.nodes.get_mut("a").unwrap().reworks_used = 2;
    h.root_usage.reworks_used = 2;
    h.project_usage.reworks_used = 2;
    h.subtree_usage.get_mut("a").unwrap().reworks_used = 2;
    assert_eq!(decision(&p, h).blocked["a"], BlockReason::RetryLimit);
}
#[test]
fn waiting_parent_releases_slot_but_holds_workspace_lock() {
    let f = Fixture::new();
    let a = f.node("a", "workspace-a");
    let mut b = f.node("b", "workspace-b");
    b.parent_id = Some("a".into());
    let p = f.plan(vec![a, b]);
    let mut h = snapshot(&p);
    observe(&p, &mut h, "a", NodeState::WaitingChild);
    f.isolate(&p, &mut h);
    assert_eq!(decision(&p, h).admissions[0].node_id, "b");
    let a = f.node("a", "workspace-a");
    let mut b = f.node("b", "workspace-a");
    b.parent_id = Some("a".into());
    let p = f.plan(vec![a, b]);
    let mut h = snapshot(&p);
    observe(&p, &mut h, "a", NodeState::WaitingChild);
    assert_eq!(decision(&p, h).blocked["b"], BlockReason::WriteConflict);
}
#[test]
fn expiry_revocation_cancel_prevent_acceptance_and_admission() {
    let f = Fixture::new();
    let p = f.plan(vec![f.node("a", "workspace-a")]);
    for mode in 0..3 {
        let mut h = snapshot(&p);
        match mode {
            0 => h.revoked = true,
            1 => h.cancelled = true,
            _ => h.now_ms = 100000,
        };
        let d = decision(&p, h);
        assert!(d.admissions.is_empty());
        assert!(!d.required_checks_satisfied);
    }
}
#[test]
fn stale_proof_and_scope_identity_cannot_be_reused_after_plan_edit() {
    let f = Fixture::new();
    let p = f.two();
    let mut h = snapshot(&p);
    f.isolate(&p, &mut h);
    h.verified_isolation[0].left.inode += 1;
    assert_eq!(
        TrustedPlanSnapshot::from_verified_host(&p, h).unwrap_err(),
        PlanError::StaleEvidence
    );
    let mut h = snapshot(&p);
    f.isolate(&p, &mut h);
    h.verified_write_sets[0].evidence.plan_digest = sha('f');
    assert_eq!(
        TrustedPlanSnapshot::from_verified_host(&p, h).unwrap_err(),
        PlanError::StaleEvidence
    );
}

#[test]
fn sibling_children_share_the_parent_cumulative_budget() {
    let f = Fixture::new();
    let mut a = f.node("a", "workspace-a");
    a.budget.attempts = 2;
    a.budget.parallel = 2;
    a.retry.max_attempts = 2;
    let mut b = f.node("b", "workspace-b");
    b.parent_id = Some("a".into());
    b.budget.attempts = 2;
    b.retry.max_attempts = 2;
    let mut c = f.node("c", "workspace-c");
    c.parent_id = Some("a".into());
    c.budget.attempts = 2;
    c.retry.max_attempts = 2;
    let p = f.plan(vec![a, b, c]);
    let mut h = snapshot(&p);
    observe(&p, &mut h, "a", NodeState::WaitingChild);
    f.isolate(&p, &mut h);
    let d = decision(&p, h);
    assert_eq!(d.admissions.len(), 1);
    assert_eq!(d.admissions[0].node_id, "b");
    assert_eq!(d.blocked["c"], BlockReason::SubtreeBudget);
}
#[test]
fn subtree_history_cannot_be_reset_by_a_fresh_plan_observation() {
    let f = Fixture::new();
    let p = f.plan(vec![f.node("a", "workspace-a")]);
    let mut h = snapshot(&p);
    h.root_usage.attempts_started = 3;
    h.project_usage = h.root_usage.clone();
    h.subtree_usage.get_mut("a").unwrap().attempts_started = 3;
    let d = decision(&p, h);
    assert!(d.admissions.is_empty());
    assert_eq!(d.blocked["a"], BlockReason::SubtreeBudget);
}
#[test]
fn child_retry_does_not_reset_root_budget_or_count_a_second_child() {
    let mut f = Fixture::new();
    f.policy.root_budget.children = 1;
    let mut a = f.node("a", "workspace-a");
    a.budget.children = 1;
    let mut b = f.node("b", "workspace-b");
    b.parent_id = Some("a".into());
    b.budget.children = 1;
    let p = f.plan(vec![a, b]);
    let mut h = snapshot(&p);
    observe(&p, &mut h, "a", NodeState::WaitingChild);
    observe(
        &p,
        &mut h,
        "b",
        NodeState::Failed {
            class: FailureClass::BeforeDispatchTransient,
            stop: StopProof::ConfirmedNotStarted,
        },
    );
    f.isolate(&p, &mut h);
    let d = decision(&p, h);
    assert_eq!(d.admissions[0].node_id, "b");
    assert_eq!(d.admissions[0].next_attempt, 2);
}
#[test]
fn subtree_underreport_and_reservation_overflow_fail_closed() {
    let f = Fixture::new();
    let p = f.plan(vec![f.node("a", "workspace-a")]);
    let mut h = snapshot(&p);
    observe(&p, &mut h, "a", NodeState::Running);
    h.subtree_usage.get_mut("a").unwrap().active_slots = 0;
    assert_eq!(
        TrustedPlanSnapshot::from_verified_host(&p, h).unwrap_err(),
        PlanError::StaleEvidence
    );
    let mut a = f.node("a", "workspace-a");
    a.reservation.runtime_ms = a.budget.runtime_ms + 1;
    assert_eq!(
        ValidatedPlan::initial(&f.scope(), f.proposal(vec![a])).unwrap_err(),
        PlanError::AuthorityExpansion
    );
}
#[test]
fn owner_approval_node_waits_for_real_host_decision() {
    let f = Fixture::new();
    let a = f.node("a", "workspace-a");
    let mut v = f.node("owner", "workspace-b");
    v.kind = NodeKind::OwnerApproval;
    v.executor = Executor::Owner {
        rule_id: "accept".into(),
    };
    v.required_validators = vec!["accept".into()];
    v.outputs.clear();
    v.required_inputs.push(input("a", false));
    let p = f.plan(vec![a, v]);
    let mut h = snapshot(&p);
    observe(&p, &mut h, "a", NodeState::ExecutionSucceeded);
    artifact(&p, &mut h, "a");
    receipt(&p, &mut h, "a", "check", true);
    let d = decision(&p, h.clone());
    assert!(d.admissions.is_empty());
    assert_eq!(d.blocked["owner"], BlockReason::AwaitingOwner);
    assert!(!d.required_checks_satisfied);
    observe(&p, &mut h, "owner", NodeState::ExecutionSucceeded);
    assert!(!decision(&p, h.clone()).required_checks_satisfied);
    receipt(&p, &mut h, "a", "accept", true);
    assert!(decision(&p, h).required_checks_satisfied);
}
#[test]
fn proposal_node_and_depth_limits_are_enforced() {
    let mut f = Fixture::new();
    f.policy.max_nodes = 1;
    assert_eq!(
        ValidatedPlan::initial(
            &f.scope(),
            f.proposal(vec![f.node("a", "workspace-a"), f.node("b", "workspace-b")])
        )
        .unwrap_err(),
        PlanError::AuthorityExpansion
    );
    let mut f = Fixture::new();
    f.policy.max_depth = 0;
    let a = f.node("a", "workspace-a");
    let mut b = f.node("b", "workspace-b");
    b.parent_id = Some("a".into());
    assert_eq!(
        ValidatedPlan::initial(&f.scope(), f.proposal(vec![a, b])).unwrap_err(),
        PlanError::Limit
    );
}

#[test]
fn deterministic_validator_cannot_be_repurposed_for_external_effects() {
    let f = Fixture::new();
    let a = f.node("a", "workspace-a");
    let mut v = f.node("validator", "workspace-b");
    v.kind = NodeKind::Validate;
    v.executor = Executor::Validator {
        rule_id: "check".into(),
    };
    v.outputs.clear();
    v.required_inputs.push(input("a", true));
    v.side_effect = SideEffect::ExternalWrite;
    v.write_set = vec![WriteTarget::External {
        resource_id: "target-1".into(),
    }];
    assert_eq!(
        ValidatedPlan::initial(&f.scope(), f.proposal(vec![a, v])).unwrap_err(),
        PlanError::InvalidContract
    );
}
#[test]
fn admission_references_exact_reservation_and_requires_atomic_commit() {
    let f = Fixture::new();
    let p = f.plan(vec![f.node("a", "workspace-a")]);
    let d = decision(&p, snapshot(&p));
    assert_eq!(d.admissions.len(), 1);
    assert_eq!(
        d.admissions[0].reservation,
        p.proposal().nodes[0].reservation
    );
    assert_eq!(d.expected_ledger_revision, 8);
    assert_eq!(d.expected_plan_revision, 1);
    assert!(d.requires_atomic_store_commit);
    assert!(!d.required_checks_satisfied);
}

fn external_node(f: &Fixture) -> PlanNode {
    let mut n = f.node("a", "workspace-a");
    n.side_effect = SideEffect::ExternalWrite;
    n.write_set = vec![WriteTarget::External {
        resource_id: "target-1".into(),
    }];
    n.external_action_id = Some("external-a".into());
    n
}
fn external_input() -> RequiredInput {
    RequiredInput {
        goal_task_id: "task-1".into(),
        artifact_id: "source".into(),
        contract_id: "result".into(),
        source: InputSource::ApprovedExternal { sha256: sha('d') },
    }
}
fn external_receipt(p: &ValidatedPlan, h: &mut HostPlanSnapshot, rule: &str) {
    h.validations.push(ValidationReceipt {
        evidence: p.evidence_binding(),
        input: resolve_for_test(p, h, &external_input()),
        rule_id: rule.into(),
        policy_digest: sha(if rule == "check" { 'b' } else { 'c' }),
        passed: true,
    });
}
#[test]
fn external_write_validation_failure_requires_reconciled_no_effect() {
    let f = Fixture::new();
    let p = f.plan(vec![external_node(&f)]);
    let first = decision(&p, snapshot(&p));
    assert_eq!(first.admissions.len(), 1);
    let mut h = snapshot(&p);
    observe(
        &p,
        &mut h,
        "a",
        NodeState::Failed {
            class: FailureClass::ValidationFailed,
            stop: StopProof::ConfirmedStopped,
        },
    );
    let stopped = decision(&p, h.clone());
    assert!(stopped.admissions.is_empty());
    assert_eq!(stopped.blocked["a"], BlockReason::ReconcileUnknown);
    h.external_actions.get_mut("external-a").unwrap().state = ExternalActionState::NoEffect;
    let safe = decision(&p, h.clone());
    assert_eq!(safe.admissions[0].next_attempt, 2);
    assert_eq!(
        safe.admissions[0].idempotency_key,
        first.admissions[0].idempotency_key
    );
    assert_eq!(
        safe.admissions[0].external_action.as_ref().unwrap().request,
        f.policy.external_actions["external-a"].request
    );
    h.external_actions.get_mut("external-a").unwrap().state = ExternalActionState::EffectConfirmed;
    assert_eq!(
        decision(&p, h).blocked["a"],
        BlockReason::ExternalEffectAlreadyApplied
    );
}
#[test]
fn before_dispatch_external_failure_can_retry_without_changing_action_key() {
    let f = Fixture::new();
    let p = f.plan(vec![external_node(&f)]);
    let first = decision(&p, snapshot(&p));
    let mut h = snapshot(&p);
    observe(
        &p,
        &mut h,
        "a",
        NodeState::Failed {
            class: FailureClass::BeforeDispatchTransient,
            stop: StopProof::ConfirmedNotStarted,
        },
    );
    let retry = decision(&p, h);
    assert_eq!(
        retry.admissions[0].idempotency_key,
        first.admissions[0].idempotency_key
    );
}
#[test]
fn external_known_effect_and_identity_survive_plan_revision_and_renaming() {
    let f = Fixture::new();
    let p = f.plan(vec![external_node(&f)]);
    let first = decision(&p, snapshot(&p));
    let mut proposal = p.proposal().clone();
    proposal.revision = 2;
    proposal.expected_revision = 1;
    proposal.nodes[0].id = "renamed".into();
    proposal.nodes[0].idempotency_key = "brand-new-model-key".into();
    let next = p.revise(&f.scope(), proposal).unwrap();
    let second = decision(&next, snapshot(&next));
    assert_eq!(
        first.admissions[0].idempotency_key,
        second.admissions[0].idempotency_key
    );
    let mut h = snapshot(&next);
    h.external_actions.get_mut("external-a").unwrap().state = ExternalActionState::EffectConfirmed;
    let d = decision(&next, h);
    assert!(d.admissions.is_empty());
    assert!(!d.required_checks_satisfied);
    assert_eq!(
        d.blocked["renamed"],
        BlockReason::ExternalEffectAlreadyApplied
    );
}
#[test]
fn external_payload_target_and_action_changes_cannot_escape_host_journal() {
    let f = Fixture::new();
    let mut n = external_node(&f);
    n.external_action_id = Some("invented-action".into());
    assert_eq!(
        ValidatedPlan::initial(&f.scope(), f.proposal(vec![n])).unwrap_err(),
        PlanError::AuthorityExpansion
    );
    let mut n = external_node(&f);
    n.write_set = vec![WriteTarget::External {
        resource_id: "target-alias".into(),
    }];
    assert_eq!(
        ValidatedPlan::initial(&f.scope(), f.proposal(vec![n])).unwrap_err(),
        PlanError::AuthorityExpansion
    );
    let mut changed = f.clone();
    changed
        .policy
        .external_actions
        .get_mut("external-a")
        .unwrap()
        .request
        .payload = serde_json::json!({"body":"changed"});
    assert_eq!(
        ApprovedGoalScope::from_verified_host(
            &changed.task,
            &changed.grant,
            changed.policy.clone(),
            10
        )
        .unwrap_err(),
        PlanError::AuthorityExpansion
    );
    let p = f.plan(vec![external_node(&f)]);
    let mut h = snapshot(&p);
    h.external_actions
        .get_mut("external-a")
        .unwrap()
        .payload_sha256 = sha('f');
    assert_eq!(
        TrustedPlanSnapshot::from_verified_host(&p, h).unwrap_err(),
        PlanError::StaleEvidence
    );
}
#[test]
fn external_journal_is_complete_root_bound_and_checks_old_actions() {
    let f = Fixture::new();
    let p = f.plan(vec![f.node("a", "workspace-a")]);
    let mut h = snapshot(&p);
    h.external_actions.clear();
    assert_eq!(
        TrustedPlanSnapshot::from_verified_host(&p, h).unwrap_err(),
        PlanError::StaleEvidence
    );
    let mut h = snapshot(&p);
    h.external_actions.get_mut("external-a").unwrap().project_id = "other-project".into();
    assert_eq!(
        TrustedPlanSnapshot::from_verified_host(&p, h).unwrap_err(),
        PlanError::StaleEvidence
    );
    for state in [
        ExternalActionState::InFlight,
        ExternalActionState::OutcomeUnknown,
    ] {
        let mut h = snapshot(&p);
        h.external_actions.get_mut("external-a").unwrap().state = state;
        assert_eq!(decision(&p, h).blocked["a"], BlockReason::ReconcileUnknown);
    }
}
#[test]
fn external_action_is_unique_and_completion_needs_confirmed_effect() {
    let f = Fixture::new();
    let a = external_node(&f);
    let mut b = a.clone();
    b.id = "b".into();
    b.outputs[0].artifact_id = "out-b".into();
    b.idempotency_key = "b-key".into();
    assert_eq!(
        ValidatedPlan::initial(&f.scope(), f.proposal(vec![a.clone(), b])).unwrap_err(),
        PlanError::AuthorityExpansion
    );
    let p = f.plan(vec![a]);
    let mut h = snapshot(&p);
    observe(&p, &mut h, "a", NodeState::ExecutionSucceeded);
    artifact(&p, &mut h, "a");
    receipt(&p, &mut h, "a", "check", true);
    receipt(&p, &mut h, "a", "accept", true);
    assert!(!decision(&p, h.clone()).required_checks_satisfied);
    h.external_actions.get_mut("external-a").unwrap().state = ExternalActionState::EffectConfirmed;
    assert!(decision(&p, h).required_checks_satisfied);
}
#[test]
fn approved_external_validator_has_a_source_exact_completed_receipt() {
    let f = Fixture::new();
    let a = f.node("a", "workspace-a");
    let mut v = f.node("verify-source", "workspace-b");
    v.kind = NodeKind::Validate;
    v.executor = Executor::Validator {
        rule_id: "check".into(),
    };
    v.outputs.clear();
    v.required_inputs = vec![external_input()];
    let p = f.plan(vec![a, v]);
    let mut h = snapshot(&p);
    observe(&p, &mut h, "a", NodeState::ExecutionSucceeded);
    artifact(&p, &mut h, "a");
    receipt(&p, &mut h, "a", "check", true);
    receipt(&p, &mut h, "a", "accept", true);
    let admission = decision(&p, h.clone());
    assert_eq!(admission.admissions[0].node_id, "verify-source");
    assert_eq!(
        admission.admissions[0].resolved_inputs[0],
        resolve_for_test(&p, &h, &external_input())
    );
    observe(&p, &mut h, "verify-source", NodeState::ExecutionSucceeded);
    assert!(!decision(&p, h.clone()).required_checks_satisfied);
    external_receipt(&p, &mut h, "check");
    assert!(decision(&p, h).required_checks_satisfied);
}
#[test]
fn external_owner_and_ordinary_consumer_require_contract_checks() {
    let f = Fixture::new();
    let mut a = f.node("a", "workspace-a");
    a.required_inputs = vec![external_input()];
    let mut owner = f.node("owner", "workspace-b");
    owner.kind = NodeKind::OwnerApproval;
    owner.executor = Executor::Owner {
        rule_id: "accept".into(),
    };
    owner.required_validators = vec!["accept".into()];
    owner.outputs.clear();
    owner.required_inputs = vec![external_input()];
    let p = f.plan(vec![a, owner]);
    let mut h = snapshot(&p);
    let blocked = decision(&p, h.clone());
    assert_eq!(blocked.blocked["a"], BlockReason::InputsNotValidated);
    assert_eq!(blocked.blocked["owner"], BlockReason::InputsNotValidated);
    external_receipt(&p, &mut h, "check");
    let d = decision(&p, h.clone());
    assert_eq!(d.admissions[0].node_id, "a");
    assert_eq!(d.blocked["owner"], BlockReason::AwaitingOwner);
    observe(&p, &mut h, "a", NodeState::ExecutionSucceeded);
    artifact(&p, &mut h, "a");
    receipt(&p, &mut h, "a", "check", true);
    receipt(&p, &mut h, "a", "accept", true);
    observe(&p, &mut h, "owner", NodeState::ExecutionSucceeded);
    assert!(!decision(&p, h.clone()).required_checks_satisfied);
    external_receipt(&p, &mut h, "accept");
    assert!(decision(&p, h).required_checks_satisfied);
}
#[test]
fn external_receipt_wrong_source_hash_scope_contract_and_collision_are_rejected() {
    let f = Fixture::new();
    let p = f.plan(vec![f.node("a", "workspace-a")]);
    let mut h = snapshot(&p);
    external_receipt(&p, &mut h, "check");
    for alter in 0..4 {
        let mut bad = h.clone();
        match alter {
            0 => {
                bad.validations[0].input.source = ResolvedInputSource::NodeOutput {
                    node_id: "a".into(),
                    attempt_id: "attempt-a".into(),
                    sha256: sha('d'),
                }
            }
            1 => {
                bad.validations[0].input.source = ResolvedInputSource::ApprovedExternal {
                    scope_revision: 2,
                    sha256: sha('e'),
                }
            }
            2 => {
                bad.validations[0].input.source = ResolvedInputSource::ApprovedExternal {
                    scope_revision: 3,
                    sha256: sha('d'),
                }
            }
            _ => bad.validations[0].input.contract_id = "wrong-contract".into(),
        };
        assert_eq!(
            TrustedPlanSnapshot::from_verified_host(&p, bad).unwrap_err(),
            PlanError::StaleEvidence
        );
    }
    let mut a = f.node("a", "workspace-a");
    a.outputs.push(PlannedOutput {
        artifact_id: "source".into(),
        contract_id: "result".into(),
    });
    assert_eq!(
        ValidatedPlan::initial(&f.scope(), f.proposal(vec![a])).unwrap_err(),
        PlanError::Duplicate
    );
}
#[test]
fn upstream_rework_invalidates_actual_downstream_consumption() {
    let mut f = Fixture::new();
    f.policy
        .required_goal_outputs
        .insert("out-b".into(), "result".into());
    let a = f.node("a", "workspace-a");
    let mut b = f.node("b", "workspace-b");
    b.dependencies.push("a".into());
    b.required_inputs.push(input("a", false));
    let p = f.plan(vec![a, b]);
    let mut h = snapshot(&p);
    observe(&p, &mut h, "a", NodeState::ExecutionSucceeded);
    artifact(&p, &mut h, "a");
    receipt(&p, &mut h, "a", "check", true);
    receipt(&p, &mut h, "a", "accept", true);
    observe(&p, &mut h, "b", NodeState::ExecutionSucceeded);
    artifact(&p, &mut h, "b");
    receipt(&p, &mut h, "b", "check", true);
    receipt(&p, &mut h, "b", "accept", true);
    assert!(decision(&p, h.clone()).required_checks_satisfied);
    h.nodes.get_mut("a").unwrap().attempts_started = 2;
    h.nodes.get_mut("a").unwrap().attempt_id = Some("attempt-a-2".into());
    observe(&p, &mut h, "b", NodeState::ExecutionSucceeded);
    // B's immutable consumed receipt remains A1/e while A2/f is verified.
    for a in &mut h.artifacts {
        if a.node_id == "a" {
            a.attempt_id = "attempt-a-2".into();
            a.sha256 = sha('f');
        }
    }
    for v in &mut h.validations {
        if let ResolvedInputSource::NodeOutput {
            node_id,
            attempt_id,
            sha256,
        } = &mut v.input.source
        {
            if node_id == "a" {
                *attempt_id = "attempt-a-2".into();
                *sha256 = sha('f');
            }
        }
    }
    let d = decision(&p, h.clone());
    assert!(!d.required_checks_satisfied);
    assert_eq!(d.blocked["b"], BlockReason::ConsumedInputsChanged);
    // Only an actual new B attempt with newly fixed inputs restores acceptance.
    observe(&p, &mut h, "b", NodeState::ExecutionSucceeded);
    h.nodes.get_mut("b").unwrap().attempt_id = Some("attempt-b-2".into());
    h.nodes.get_mut("b").unwrap().attempts_started = 2;
    for c in &mut h.nodes.get_mut("b").unwrap().consumed_inputs {
        c.consumer_attempt_id = "attempt-b-2".into();
    }
    for a in &mut h.artifacts {
        if a.node_id == "b" {
            a.attempt_id = "attempt-b-2".into();
        }
    }
    for v in &mut h.validations {
        if let ResolvedInputSource::NodeOutput {
            node_id,
            attempt_id,
            ..
        } = &mut v.input.source
        {
            if node_id == "b" {
                *attempt_id = "attempt-b-2".into();
            }
        }
    }
    h.root_usage.attempts_started = 4;
    h.project_usage.attempts_started = 4;
    h.subtree_usage.get_mut("b").unwrap().attempts_started = 2;
    assert!(decision(&p, h).required_checks_satisfied);
}
#[test]
fn consumed_receipts_are_bound_to_consumer_attempt_and_exact_external_source() {
    let f = Fixture::new();
    let mut a = f.node("a", "workspace-a");
    a.required_inputs = vec![external_input()];
    let p = f.plan(vec![a]);
    let mut h = snapshot(&p);
    observe(&p, &mut h, "a", NodeState::ExecutionSucceeded);
    for alter in 0..4 {
        let mut bad = h.clone();
        let c = &mut bad.nodes.get_mut("a").unwrap().consumed_inputs[0];
        match alter {
            0 => c.consumer_node_id = "another".into(),
            1 => c.consumer_attempt_id = "old-attempt".into(),
            2 => c.input.contract_id = "wrong".into(),
            _ => {
                c.input.source = ResolvedInputSource::ApprovedExternal {
                    scope_revision: 2,
                    sha256: sha('f'),
                }
            }
        };
        assert_eq!(
            TrustedPlanSnapshot::from_verified_host(&p, bad).unwrap_err(),
            PlanError::StaleEvidence
        );
    }
}

#[test]
fn same_semantic_payload_cannot_gain_another_host_action_id_via_resource_alias() {
    let mut f = Fixture::new();
    let body = f.policy.external_actions["external-a"]
        .request
        .payload
        .clone();
    let b = f.policy.external_actions.get_mut("external-b").unwrap();
    b.request.payload = body;
    *b = ApprovedExternalAction::from_verified_host(b.executor.clone(), b.request.clone()).unwrap();
    assert_eq!(
        ApprovedGoalScope::from_verified_host(&f.task, &f.grant, f.policy, 10).unwrap_err(),
        PlanError::Duplicate
    );
}
#[test]
fn input_invalidation_propagates_to_grandchildren_and_candidate_validators() {
    let f = Fixture::new();
    let a = f.node("a", "workspace-a");
    let mut b = f.node("b", "workspace-b");
    b.required_inputs.push(input("a", false));
    let mut c = f.node("c", "workspace-c");
    c.required_inputs.push(input("b", false));
    let mut v = f.node("validator", "workspace-c");
    v.kind = NodeKind::Validate;
    v.executor = Executor::Validator {
        rule_id: "check".into(),
    };
    v.outputs.clear();
    v.required_inputs.push(input("a", true));
    let p = f.plan(vec![a, b, c, v]);
    let mut h = snapshot(&p);
    for n in ["a", "b", "c"] {
        observe(&p, &mut h, n, NodeState::ExecutionSucceeded);
        artifact(&p, &mut h, n);
        receipt(&p, &mut h, n, "check", true);
        receipt(&p, &mut h, n, "accept", true);
    }
    observe(&p, &mut h, "validator", NodeState::ExecutionSucceeded);
    assert!(decision(&p, h.clone()).required_checks_satisfied);
    // Same producer attempt with changed SHA is also stale, not just attempt IDs.
    h.artifacts
        .iter_mut()
        .find(|a| a.node_id == "a")
        .unwrap()
        .sha256 = sha('f');
    for r in &mut h.validations {
        if let ResolvedInputSource::NodeOutput {
            node_id, sha256, ..
        } = &mut r.input.source
        {
            if node_id == "a" {
                *sha256 = sha('f');
            }
        }
    }
    let d = decision(&p, h);
    assert!(!d.required_checks_satisfied);
    for n in ["b", "c", "validator"] {
        assert_eq!(d.blocked[n], BlockReason::ConsumedInputsChanged);
    }
}
