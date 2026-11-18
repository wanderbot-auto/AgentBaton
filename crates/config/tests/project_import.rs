use agentbaton_config::{
    capabilities::ProviderRevision,
    project_import::{
        apply_project_import, inspect_project_config, prepare_project_import,
        read_imported_project_plan, recover_project_import, ConfirmedProjectImport,
        ProjectCapabilityAdmission as Admission, ProjectCapabilityKind as Kind,
        ProjectImportBinding, ProjectImportChoice as Choice, ProjectImportDecision as Decision,
        ProjectImportPolicy, ProjectImportRequest, ProjectImportScope, ProjectImportState as State,
        ProjectSource,
    },
    runtime::{
        ManagedMcpServer, ManagedMcpTransport, NonSecretText, RuntimeRequirements, RuntimeRoot,
        SourceProvenance,
    },
    sha256, ErrorCode, Ownership,
};
use serde_json::{json, Value};
use std::{
    fs,
    os::unix::{
        ffi::OsStrExt,
        fs::{symlink, MetadataExt, PermissionsExt},
    },
    path::{Path, PathBuf},
    process::Command,
};

const PROPOSAL: &str = "99999999-1234-4234-8234-123456789abc";
const SKILL: &str = "---\nname: review\ndescription: Review a fixture report\n---\nReview the provided plain-text report.\n";
const CANARY: &str = "sensitive-project-token-value";

struct Fixture {
    private: tempfile::TempDir,
    scope: ProjectImportScope,
    policy: ProjectImportPolicy,
    managed: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let private = tempfile::tempdir().unwrap();
        let root = private.path().join("source");
        let managed = private.path().join("managed");
        fs::create_dir(&root).unwrap();
        fs::create_dir(&managed).unwrap();
        fs::set_permissions(&managed, fs::Permissions::from_mode(0o700)).unwrap();
        let root = root.canonicalize().unwrap();
        let managed = managed.canonicalize().unwrap();
        git(&root, &["init", "--initial-branch=main"]);
        let marker = private.path().join("project-command-ran");
        put(&root, ".claude/skills/review/SKILL.md", SKILL.as_bytes());
        put(&root, ".claude/settings.json", &serde_json::to_vec(&json!({
            "hooks": { "PreToolUse": [{"hooks": [{"type": "command", "command": format!("touch {}", marker.display())}]}] },
            "env": {"EXAMPLE_TOKEN": CANARY},
            "unknown_setting": CANARY
        })).unwrap());
        put(
            &root,
            ".mcp.json",
            &serde_json::to_vec(&json!({
                "mcpServers": {
                    "fixture": {"command": "ignored-project-command", "args": [CANARY]},
                    "secret": {"command": "ignored-project-command", "env": {"TOKEN": CANARY}},
                    "other": {"type": "sse", "url": format!("https://127.0.0.1:1/{CANARY}")}
                }
            }))
            .unwrap(),
        );
        put(&root, "auth.json", CANARY.as_bytes());
        git(&root, &["add", "."]);
        git(&root, &["commit", "-m", "Synthetic project inputs"]);
        let metadata = fs::metadata(&root).unwrap();
        let scope = ProjectImportScope {
            project_id: "registered-project".into(),
            project_revision: 4,
            input_revision: git(&root, &["rev-parse", "HEAD"]),
            root,
            root_device: metadata.dev(),
            root_inode: metadata.ino(),
        };
        let policy = ProjectImportPolicy {
            revision: "fixture-policy-v1".into(),
            provider: ProviderRevision {
                provider_id: "claude-code".into(),
                version: "fixture-version-1".into(),
            },
            sources: vec![
                ProjectSource::ClaudeSkills,
                ProjectSource::ClaudeMcp,
                ProjectSource::ClaudeSettings,
                ProjectSource::ClaudeLocalSettings,
            ],
        };
        Self {
            private,
            scope,
            policy,
            managed,
        }
    }

    fn request<'a>(&'a self, choices: &'a [Choice<'a>]) -> ProjectImportRequest<'a> {
        ProjectImportRequest {
            proposal_id: PROPOSAL,
            scope: &self.scope,
            policy: &self.policy,
            managed_project_root: &self.managed,
            choices,
        }
    }

    fn no_writes(&self) {
        assert_eq!(fs::read_dir(&self.managed).unwrap().count(), 0);
        assert!(!self.private.path().join("project-command-ran").exists());
    }
}

fn put(root: &Path, file: &str, bytes: &[u8]) {
    let path = root.join(file);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, bytes).unwrap();
}

fn git(root: &Path, args: &[&str]) -> String {
    let mut command = Command::new("git");
    for (key, _) in std::env::vars_os() {
        if key.as_bytes().starts_with(b"GIT_") {
            command.env_remove(key);
        }
    }
    let output = command
        .arg("-C")
        .arg(root)
        .args([
            "-c",
            "user.name=Config fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "-c",
            "core.hooksPath=/dev/null",
            "-c",
            "commit.gpgsign=false",
        ])
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_TERMINAL_PROMPT", "0")
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().into()
}

fn approval(binding: &ProjectImportBinding) -> ConfirmedProjectImport<'_> {
    ConfirmedProjectImport {
        approval_ref: "confirmed-by-trusted-test-owner",
        binding,
    }
}

fn registered_mcp(name: &str) -> ManagedMcpServer {
    ManagedMcpServer {
        name: name.into(),
        codex_policy: None,
        transport: ManagedMcpTransport::Stdio {
            command: NonSecretText::approved("/trusted/registered/fixture".into()).unwrap(),
            arguments: vec![],
        },
        provenance: SourceProvenance {
            source_ref: "registry:registered-mcp".into(),
            revision: "1".into(),
            sha256: sha256(b"validated managed connection"),
        },
    }
}

#[test]
fn inspect_cancel_and_skip_do_not_write_or_execute_anything() {
    let f = Fixture::new();
    let inspection = inspect_project_config(&f.scope, &f.policy).unwrap();
    assert!(inspection.sources.iter().any(|source| source.source
        == ProjectSource::ClaudeLocalSettings
        && source.sha256.is_none()));
    assert!(inspection
        .candidates
        .iter()
        .any(|item| item.key.kind == Kind::Hook
            && item.name == "PreToolUse"
            && item.admission == Admission::Unsupported
            && !item.runtime_loading_known));
    assert!(inspection
        .candidates
        .iter()
        .any(|item| item.name == "secret" && item.admission == Admission::ExcludedSensitiveFields));
    let inspect_json = serde_json::to_string(&inspection).unwrap();
    assert!(!inspect_json.contains(CANARY));
    assert!(!inspect_json.contains("ignored-project-command"));
    assert!(!inspect_json.contains("touch "));
    let request = f.request(&[]);
    let review = prepare_project_import(&request).unwrap();
    assert!(review.new_runtime_required);
    assert_eq!(
        recover_project_import(&f.managed, &approval(&review.binding)).unwrap(),
        State::NotApplied
    );
    assert_eq!(
        apply_project_import(&request, Decision::Skip).unwrap(),
        State::Skipped
    );
    assert_eq!(
        apply_project_import(&request, Decision::Cancel).unwrap(),
        State::Cancelled
    );
    f.no_writes();
}

#[test]
fn confirmation_materializes_only_classified_instructions_and_registered_mcp_then_reopens() {
    let f = Fixture::new();
    let inspection = inspect_project_config(&f.scope, &f.policy).unwrap();
    let skill = inspection
        .candidates
        .iter()
        .find(|item| item.name == "review")
        .unwrap();
    let candidate = inspection
        .candidates
        .iter()
        .find(|item| item.name == "fixture")
        .unwrap();
    assert_eq!(skill.admission, Admission::ReviewedInstruction);
    assert_eq!(candidate.admission, Admission::RegisteredMcpOnly);
    let server = registered_mcp("fixture");
    let choices = [
        Choice::Skill {
            key: &skill.key,
            nonsecret_sha256: &skill.source_sha256,
        },
        Choice::RegisteredMcp {
            key: &candidate.key,
            definition: &server,
        },
    ];
    let request = f.request(&choices);
    let review = prepare_project_import(&request).unwrap();
    assert_eq!(review.selections.len(), 2);
    assert!(review
        .selections
        .iter()
        .any(|entry| entry.managed_provenance.source_ref == "registry:registered-mcp"));
    f.no_writes();
    assert_eq!(
        apply_project_import(&request, Decision::Confirm(approval(&review.binding))).unwrap(),
        State::Applied
    );
    let restored: ProjectImportBinding =
        serde_json::from_slice(&serde_json::to_vec(&review.binding).unwrap()).unwrap();
    assert_eq!(
        recover_project_import(&f.managed, &approval(&restored)).unwrap(),
        State::Applied
    );
    let file = f.managed.join("project-config.json");
    let bytes = fs::read(&file).unwrap();
    assert_eq!(sha256(&bytes), restored.snapshot_sha256);
    assert_eq!(fs::metadata(&file).unwrap().mode() & 0o777, 0o600);
    let text = String::from_utf8(bytes.clone()).unwrap();
    for excluded in [CANARY, "ignored-project-command", "touch ", "auth.json"] {
        assert!(!text.contains(excluded), "{excluded}");
    }
    let plan = read_imported_project_plan(&f.managed, &approval(&restored))
        .unwrap()
        .unwrap();
    assert_eq!(plan.plan_digest().unwrap(), review.runtime_plan_sha256);
    assert_eq!(plan.skills[0].instruction_text(), SKILL);
    assert!(plan.skills[0].assets.is_empty());
    assert_eq!(
        plan.mcp_servers[0].provenance.source_ref,
        "registry:registered-mcp"
    );
    let runtime = f.private.path().join("runtime");
    fs::create_dir(&runtime).unwrap();
    fs::set_permissions(&runtime, fs::Permissions::from_mode(0o700)).unwrap();
    let root = RuntimeRoot::open(&runtime.canonicalize().unwrap()).unwrap();
    let snapshot = root.materialize(plan, Ownership::BatonManaged).unwrap();
    let verified = root
        .verify_before_launch(&snapshot.descriptor(), RuntimeRequirements::default())
        .unwrap();
    let runtime_json = verified.acp_configuration().unwrap();
    assert_eq!(
        runtime_json["mcpServers"][0]["command"],
        "/trusted/registered/fixture"
    );
    assert!(!runtime_json.to_string().contains(CANARY));
    assert!(!runtime_json.to_string().contains("PreToolUse"));

    put(
        &f.scope.root,
        ".claude/settings.local.json",
        br#"{"hooks":{}}"#,
    );
    assert_eq!(
        apply_project_import(&request, Decision::Confirm(approval(&restored))).unwrap(),
        State::Applied
    );
    assert_eq!(fs::read(file).unwrap(), bytes);
    assert_eq!(
        fs::read_dir(f.managed.join(".baton-transactions"))
            .unwrap()
            .count(),
        1
    );
    assert!(!f.private.path().join("project-command-ran").exists());
    assert_eq!(
        fs::read_to_string(f.scope.root.join("auth.json")).unwrap(),
        CANARY
    );
}

#[test]
fn binding_rejects_source_and_review_drift_before_creating_a_transaction() {
    let f = Fixture::new();
    let inspection = inspect_project_config(&f.scope, &f.policy).unwrap();
    let item = inspection
        .candidates
        .iter()
        .find(|item| item.name == "review")
        .unwrap();
    let choices = [Choice::Skill {
        key: &item.key,
        nonsecret_sha256: &item.source_sha256,
    }];
    let request = f.request(&choices);
    let review = prepare_project_import(&request).unwrap();
    assert_eq!(
        apply_project_import(
            &request,
            Decision::Confirm(ConfirmedProjectImport {
                approval_ref: "",
                binding: &review.binding,
            })
        )
        .unwrap_err()
        .code,
        ErrorCode::PermissionDenied,
    );
    let alternate = ProjectImportPolicy {
        revision: "different-policy".into(),
        ..f.policy.clone()
    };
    let new_policy = ProjectImportRequest {
        policy: &alternate,
        ..f.request(&choices)
    };
    let trimmed = f.request(&[]);
    let other = ProjectImportScope {
        project_revision: f.scope.project_revision + 1,
        ..f.scope.clone()
    };
    let later = ProjectImportRequest {
        scope: &other,
        ..f.request(&choices)
    };
    git(
        &f.scope.root,
        &[
            "commit",
            "--allow-empty",
            "-m",
            "New trusted input revision",
        ],
    );
    let new_input = ProjectImportScope {
        input_revision: git(&f.scope.root, &["rev-parse", "HEAD"]),
        ..f.scope.clone()
    };
    let later_input = ProjectImportRequest {
        scope: &new_input,
        ..f.request(&choices)
    };
    for changed in [&new_policy, &trimmed, &later, &later_input] {
        assert_eq!(
            apply_project_import(changed, Decision::Confirm(approval(&review.binding)))
                .unwrap_err()
                .code,
            ErrorCode::Conflict
        );
    }
    put(
        &f.scope.root,
        ".claude/settings.local.json",
        br#"{"hooks":{}}"#,
    );
    assert_eq!(
        apply_project_import(&request, Decision::Confirm(approval(&review.binding)))
            .unwrap_err()
            .code,
        ErrorCode::Conflict
    );
    fs::remove_file(f.scope.root.join(".claude/settings.local.json")).unwrap();
    put(
        &f.scope.root,
        ".claude/skills/review/SKILL.md",
        SKILL.replace("plain-text", "changed").as_bytes(),
    );
    assert_eq!(
        apply_project_import(&request, Decision::Confirm(approval(&review.binding)))
            .unwrap_err()
            .code,
        ErrorCode::Conflict
    );
    f.no_writes();
}

#[test]
fn conflict_requires_one_selection_and_unsupported_or_sensitive_content_is_rejected() {
    let f = Fixture::new();
    put(
        &f.scope.root,
        ".claude/skills/duplicate/SKILL.md",
        SKILL.as_bytes(),
    );
    put(
        &f.scope.root,
        ".claude/skills/review/scripts/helper.sh",
        b"exit 9",
    );
    put(
        &f.scope.root,
        ".claude/skills/second/SKILL.md",
        SKILL.as_bytes(),
    );
    let inspection = inspect_project_config(&f.scope, &f.policy).unwrap();
    let skills: Vec<_> = inspection
        .candidates
        .iter()
        .filter(|item| item.key.kind == Kind::Skill)
        .collect();
    assert_eq!(skills.len(), 3);
    assert!(skills.iter().all(|item| item.conflict_count == 2));
    let selectable: Vec<_> = skills
        .iter()
        .filter(|item| item.admission == Admission::ReviewedInstruction)
        .collect();
    assert_eq!(selectable.len(), 2);
    let duplicate: Vec<_> = selectable
        .iter()
        .map(|item| Choice::Skill {
            key: &item.key,
            nonsecret_sha256: &item.source_sha256,
        })
        .collect();
    assert_eq!(
        prepare_project_import(&f.request(&duplicate))
            .unwrap_err()
            .code,
        ErrorCode::Conflict
    );
    assert!(prepare_project_import(&f.request(&duplicate[..1])).is_ok());
    let scripted = skills
        .iter()
        .find(|item| item.key.locator == "review")
        .unwrap();
    let rejected_script = [Choice::Skill {
        key: &scripted.key,
        nonsecret_sha256: &scripted.source_sha256,
    }];
    assert_eq!(
        prepare_project_import(&f.request(&rejected_script))
            .unwrap_err()
            .code,
        ErrorCode::PermissionDenied
    );
    let sensitive = inspection
        .candidates
        .iter()
        .find(|item| item.name == "secret")
        .unwrap();
    let server = registered_mcp("secret");
    let rejected_secret = [Choice::RegisteredMcp {
        key: &sensitive.key,
        definition: &server,
    }];
    assert_eq!(
        prepare_project_import(&f.request(&rejected_secret))
            .unwrap_err()
            .code,
        ErrorCode::PermissionDenied
    );
    let wrong_hash = [Choice::Skill {
        key: &selectable[0].key,
        nonsecret_sha256: &"1".repeat(64),
    }];
    assert_eq!(
        prepare_project_import(&f.request(&wrong_hash))
            .unwrap_err()
            .code,
        ErrorCode::Conflict
    );
    f.no_writes();
}

#[test]
fn project_and_managed_root_aliases_and_unsafe_locators_are_rejected() {
    let f = Fixture::new();
    let request = f.request(&[]);
    let review = prepare_project_import(&request).unwrap();
    let sibling = f.private.path().join("sibling");
    fs::create_dir(&sibling).unwrap();
    fs::set_permissions(&sibling, fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(
        recover_project_import(&sibling.canonicalize().unwrap(), &approval(&review.binding))
            .unwrap_err()
            .code,
        ErrorCode::Conflict
    );
    let in_source = ProjectImportRequest {
        managed_project_root: &f.scope.root,
        ..f.request(&[])
    };
    assert_eq!(
        prepare_project_import(&in_source).unwrap_err().code,
        ErrorCode::PermissionDenied
    );
    let alias = f.private.path().join("alias");
    symlink(&f.scope.root, &alias).unwrap();
    let wrong_scope = ProjectImportScope {
        root: alias,
        ..f.scope.clone()
    };
    assert_eq!(
        inspect_project_config(&wrong_scope, &f.policy)
            .unwrap_err()
            .code,
        ErrorCode::InvalidPath
    );
    symlink(
        f.scope.root.join(".claude/settings.json"),
        f.scope.root.join(".claude/settings.local.json"),
    )
    .unwrap();
    assert_eq!(
        inspect_project_config(&f.scope, &f.policy)
            .unwrap_err()
            .code,
        ErrorCode::PermissionDenied
    );
    f.no_writes();
}

#[test]
fn recovery_distinguishes_unapplied_from_unknown_foreign_snapshot_or_incomplete_journal() {
    let f = Fixture::new();
    let request = f.request(&[]);
    let review = prepare_project_import(&request).unwrap();
    let binding = &review.binding;
    assert!(read_imported_project_plan(&f.managed, &approval(binding))
        .unwrap()
        .is_none());
    fs::write(f.managed.join("project-config.json"), b"other content").unwrap();
    fs::set_permissions(
        f.managed.join("project-config.json"),
        fs::Permissions::from_mode(0o600),
    )
    .unwrap();
    assert_eq!(
        apply_project_import(&request, Decision::Confirm(approval(binding))).unwrap(),
        State::NeedsReconciliation
    );
    assert_eq!(
        fs::read(f.managed.join("project-config.json")).unwrap(),
        b"other content"
    );
    fs::remove_file(f.managed.join("project-config.json")).unwrap();
    let journal = f.managed.join(".baton-transactions").join(PROPOSAL);
    fs::create_dir_all(&journal).unwrap();
    assert_eq!(
        recover_project_import(&f.managed, &approval(binding)).unwrap(),
        State::NeedsReconciliation
    );
    assert_eq!(
        apply_project_import(&request, Decision::Confirm(approval(binding))).unwrap(),
        State::NeedsReconciliation
    );
    assert!(!f.managed.join("project-config.json").exists());
}

#[test]
fn destination_drift_after_confirmation_never_changes_the_original() {
    let f = Fixture::new();
    let request = f.request(&[]);
    let review = prepare_project_import(&request).unwrap();
    let binding = &review.binding;
    assert_eq!(
        apply_project_import(&request, Decision::Confirm(approval(binding))).unwrap(),
        State::Applied
    );
    let file = f.managed.join("project-config.json");
    let mut changed: Value = serde_json::from_slice(&fs::read(&file).unwrap()).unwrap();
    changed["policy"]["revision"] = json!("later-edited");
    fs::write(&file, serde_json::to_vec(&changed).unwrap()).unwrap();
    assert_eq!(
        recover_project_import(&f.managed, &approval(binding)).unwrap(),
        State::NeedsReconciliation
    );
    assert_eq!(
        read_imported_project_plan(&f.managed, &approval(binding))
            .unwrap_err()
            .code,
        ErrorCode::OutcomeUnknown
    );
    assert_eq!(
        apply_project_import(&request, Decision::Confirm(approval(binding))).unwrap(),
        State::NeedsReconciliation
    );
    let after: Value = serde_json::from_slice(&fs::read(&file).unwrap()).unwrap();
    assert_eq!(after, changed);
}
