use agentbaton_config::{
    capabilities::ProviderRevision,
    project_import::{
        apply_project_config_update, apply_project_import, inspect_project_config,
        prepare_project_config_update, prepare_project_config_update_binding,
        prepare_project_import, read_imported_project_plan, read_project_config_version,
        recover_project_config_update, recover_project_import, ConfirmedProjectConfigUpdate,
        ConfirmedProjectImport, ProjectCapabilityAdmission as Admission, ProjectCapabilityKey,
        ProjectCapabilityKind as Kind, ProjectConfigUpdateBinding, ProjectConfigUpdatePreview,
        ProjectConfigUpdateRequest, ProjectConfigUpdateReviewReason as ReviewReason,
        ProjectConfigUpdateRule, ProjectConfigVersionBinding, ProjectImportBinding,
        ProjectImportChoice as Choice, ProjectImportDecision as Decision, ProjectImportPolicy,
        ProjectImportRequest, ProjectImportScope, ProjectImportState as State, ProjectSource,
    },
    runtime::{
        ManagedMcpServer, ManagedMcpTransport, NonSecretText, RuntimeRequirements, RuntimeRoot,
        SourceProvenance,
    },
    sha256, ErrorCode, Ownership,
};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
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
    let other_project = ProjectImportBinding {
        project_id: "different-project".into(),
        ..binding.clone()
    };
    assert_eq!(
        recover_project_import(&f.managed, &approval(&other_project)).unwrap(),
        State::NeedsReconciliation
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

const UPDATE_ONE: &str = "99999999-1234-4234-8234-123456789abd";
const UPDATE_TWO: &str = "99999999-1234-4234-8234-123456789abe";
const UPDATE_THREE: &str = "99999999-1234-4234-8234-123456789abf";
const RESTART_RECEIPT: &str = "AGENTBATON_TEST_DRIFT_RESTART_RECEIPT";
const PATH_TO_SKILL: &str = ".claude/skills/review/SKILL.md";

struct DriftFixture {
    first: ProjectImportBinding,
    binding: ProjectConfigUpdateBinding,
    rule: ProjectConfigUpdateRule,
    key: ProjectCapabilityKey,
}

impl DriftFixture {
    fn accept(f: &Fixture) -> Self {
        let view = inspect_project_config(&f.scope, &f.policy).unwrap();
        let key = view
            .candidates
            .iter()
            .find(|item| item.key.kind == Kind::Skill)
            .unwrap()
            .key
            .clone();
        let mcp = view
            .candidates
            .iter()
            .find(|item| item.name == "fixture")
            .unwrap();
        let registered = registered_mcp("fixture");
        let choices = [
            Choice::Skill {
                key: &key,
                nonsecret_sha256: &sha256(SKILL.as_bytes()),
            },
            Choice::RegisteredMcp {
                key: &mcp.key,
                definition: &registered,
            },
        ];
        let request = f.request(&choices);
        let first = prepare_project_import(&request).unwrap().binding;
        assert_eq!(
            apply_project_import(&request, Decision::Confirm(approval(&first))).unwrap(),
            State::Applied
        );
        let rule = ProjectConfigUpdateRule {
            revision: "selected-safe-instructions-v1".into(),
            instruction_keys: BTreeSet::from([key.clone()]),
        };
        let binding =
            prepare_project_config_update_binding(&f.managed, &approval(&first), &rule).unwrap();
        Self {
            first,
            binding,
            rule,
            key,
        }
    }

    fn approval(&self) -> ConfirmedProjectConfigUpdate<'_> {
        ConfirmedProjectConfigUpdate {
            approval_ref: "confirmed-by-trusted-test-owner",
            binding: &self.binding,
            rule: &self.rule,
        }
    }

    fn classified(&self, instruction: &str) -> BTreeMap<ProjectCapabilityKey, String> {
        BTreeMap::from([(self.key.clone(), sha256(instruction.as_bytes()))])
    }
}

fn drift_request<'a>(
    f: &'a Fixture,
    id: &'a str,
    scope: &'a ProjectImportScope,
    base: Option<&'a ProjectConfigVersionBinding>,
    classified: &'a BTreeMap<ProjectCapabilityKey, String>,
) -> ProjectConfigUpdateRequest<'a> {
    ProjectConfigUpdateRequest {
        transaction_id: id,
        managed_project_root: &f.managed,
        scope,
        policy: &f.policy,
        base,
        classified_instructions: classified,
    }
}

fn ready(result: ProjectConfigUpdatePreview) -> ProjectConfigVersionBinding {
    let ProjectConfigUpdatePreview::Ready(binding) = result else {
        panic!("expected a prepared update, got {result:?}");
    };
    binding
}

fn revised_instruction(body: &str) -> String {
    SKILL.replace("Review the provided plain-text report.", body)
}

fn commit_skill(f: &Fixture, content: &str) -> ProjectImportScope {
    put(&f.scope.root, PATH_TO_SKILL, content.as_bytes());
    git(&f.scope.root, &["add", PATH_TO_SKILL]);
    git(
        &f.scope.root,
        &["commit", "-m", "Revise the selected instruction"],
    );
    let mut scope = f.scope.clone();
    scope.input_revision = git(&scope.root, &["rev-parse", "HEAD"]);
    scope
}

fn journal_count(f: &Fixture) -> usize {
    fs::read_dir(f.managed.join(".baton-transactions"))
        .unwrap()
        .count()
}

#[test]
fn approved_drift_creates_immutable_versions_and_reopens_in_a_fresh_process() {
    if let Some(receipt) = std::env::var_os(RESTART_RECEIPT) {
        // This branch runs in a new instance of the integration test binary.
        let data: Value = serde_json::from_slice(&fs::read(receipt).unwrap()).unwrap();
        let binding = serde_json::from_value(data["binding"].clone()).unwrap();
        let rule = serde_json::from_value(data["rule"].clone()).unwrap();
        let managed = Path::new(data["managed"].as_str().unwrap());
        let approval = ConfirmedProjectConfigUpdate {
            approval_ref: "confirmed-by-trusted-test-owner",
            binding: &binding,
            rule: &rule,
        };
        for entry in data["versions"].as_array().unwrap() {
            let version = serde_json::from_value(entry["version"].clone()).unwrap();
            assert_eq!(
                recover_project_config_update(managed, &approval, &version).unwrap(),
                State::Applied
            );
            let plan = read_project_config_version(managed, &approval, &version)
                .unwrap()
                .unwrap();
            assert_eq!(
                plan.skills[0].instruction_text(),
                entry["text"].as_str().unwrap()
            );
        }
        return;
    }
    let f = Fixture::new();
    let drift = DriftFixture::accept(&f);
    let original = fs::read(f.managed.join("project-config.json")).unwrap();
    let original_settings = fs::read(f.scope.root.join(".claude/settings.json")).unwrap();
    let original_plan = read_imported_project_plan(&f.managed, &approval(&drift.first))
        .unwrap()
        .unwrap();
    let empty = BTreeMap::new();
    let initial = drift_request(&f, UPDATE_ONE, &f.scope, None, &empty);
    assert_eq!(
        prepare_project_config_update(&initial, &drift.approval()).unwrap(),
        ProjectConfigUpdatePreview::Unchanged
    );
    let mut same_input = f.scope.clone();
    same_input.input_revision = "unrelated-new-input-revision".into();
    let same = drift_request(&f, UPDATE_ONE, &same_input, None, &empty);
    assert_eq!(
        prepare_project_config_update(&same, &drift.approval()).unwrap(),
        ProjectConfigUpdatePreview::Unchanged
    );
    assert_eq!(journal_count(&f), 1);

    let version_one_text = revised_instruction("Summarize the plain-text report.");
    let scope = commit_skill(&f, &version_one_text);
    let unclassified = drift_request(&f, UPDATE_ONE, &scope, None, &empty);
    assert_eq!(
        prepare_project_config_update(&unclassified, &drift.approval()).unwrap(),
        ProjectConfigUpdatePreview::ReviewRequired(
            ReviewReason::InstructionClassificationRequired(drift.key.clone())
        )
    );
    let classified = drift.classified(&version_one_text);
    let request = drift_request(&f, UPDATE_ONE, &scope, None, &classified);
    let v1 = ready(prepare_project_config_update(&request, &drift.approval()).unwrap());
    assert_eq!(journal_count(&f), 1);
    assert_eq!(
        recover_project_config_update(&f.managed, &drift.approval(), &v1).unwrap(),
        State::NotApplied
    );
    assert!(
        read_project_config_version(&f.managed, &drift.approval(), &v1)
            .unwrap()
            .is_none()
    );
    assert_eq!(
        apply_project_config_update(&request, &drift.approval(), &v1).unwrap(),
        State::Applied
    );
    assert_eq!(
        apply_project_config_update(&request, &drift.approval(), &v1).unwrap(),
        State::Applied
    );
    assert_eq!(journal_count(&f), 2);
    let old_slot = f
        .managed
        .join(format!("project-config-next-{}.json", v1.parent_sha256));
    let saved_v1 = fs::read(&old_slot).unwrap();
    assert_eq!(sha256(&saved_v1), v1.snapshot_sha256);
    assert_eq!(fs::metadata(&old_slot).unwrap().mode() & 0o7777, 0o600);
    assert!(!String::from_utf8_lossy(&saved_v1).contains(CANARY));
    let plan_one = read_project_config_version(&f.managed, &drift.approval(), &v1)
        .unwrap()
        .unwrap();
    assert_eq!(plan_one.skills[0].instruction_text(), version_one_text);
    assert_eq!(plan_one.skills[0].provenance.revision, scope.input_revision);
    assert_eq!(
        serde_json::to_value(&plan_one.mcp_servers).unwrap(),
        serde_json::to_value(&original_plan.mcp_servers).unwrap()
    );
    let nothing_more = drift_request(&f, UPDATE_TWO, &scope, Some(&v1), &empty);
    assert_eq!(
        prepare_project_config_update(&nothing_more, &drift.approval()).unwrap(),
        ProjectConfigUpdatePreview::Unchanged
    );

    let version_two_text = revised_instruction("Summarize the report and include its date.");
    let scope_two = commit_skill(&f, &version_two_text);
    let classified_two = drift.classified(&version_two_text);
    let next = drift_request(&f, UPDATE_TWO, &scope_two, Some(&v1), &classified_two);
    let v2 = ready(prepare_project_config_update(&next, &drift.approval()).unwrap());
    assert_eq!(
        apply_project_config_update(&next, &drift.approval(), &v2).unwrap(),
        State::Applied
    );
    assert_eq!(journal_count(&f), 3);
    let stale = drift_request(&f, UPDATE_THREE, &scope_two, None, &classified_two);
    assert_eq!(
        prepare_project_config_update(&stale, &drift.approval()).unwrap(),
        ProjectConfigUpdatePreview::ReviewRequired(ReviewReason::StaleBase)
    );
    let newest = drift_request(&f, UPDATE_THREE, &scope_two, Some(&v2), &empty);
    assert_eq!(
        prepare_project_config_update(&newest, &drift.approval()).unwrap(),
        ProjectConfigUpdatePreview::Unchanged
    );
    assert_eq!(fs::read(&old_slot).unwrap(), saved_v1);
    assert_eq!(
        fs::read(f.managed.join("project-config.json")).unwrap(),
        original
    );
    assert_eq!(
        read_imported_project_plan(&f.managed, &approval(&drift.first))
            .unwrap()
            .unwrap()
            .skills[0]
            .instruction_text(),
        SKILL
    );
    assert_eq!(
        fs::read(f.scope.root.join(PATH_TO_SKILL)).unwrap(),
        version_two_text.as_bytes()
    );
    assert_eq!(
        fs::read(f.scope.root.join(".claude/settings.json")).unwrap(),
        original_settings
    );
    assert_eq!(
        fs::read(f.scope.root.join("auth.json")).unwrap(),
        CANARY.as_bytes()
    );
    assert!(!f.private.path().join("project-command-ran").exists());

    // Leave the second durable intent in the state reachable after publication
    // but before the final report is recorded.
    let manifest = f
        .managed
        .join(".baton-transactions")
        .join(&v2.transaction_id)
        .join("manifest.json");
    let mut report: Value = serde_json::from_slice(&fs::read(&manifest).unwrap()).unwrap();
    report["phase"] = json!("Applying");
    report["files"][0]["state"] = json!("Pending");
    fs::write(&manifest, serde_json::to_vec(&report).unwrap()).unwrap();
    let receipt = f.private.path().join("restart-receipt.json");
    fs::write(&receipt, serde_json::to_vec(&json!({
        "binding": drift.binding,
        "rule": drift.rule,
        "managed": f.managed,
        "versions": [{"version": v1, "text": version_one_text}, {"version": v2, "text": version_two_text}]
    })).unwrap()).unwrap();
    let child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "approved_drift_creates_immutable_versions_and_reopens_in_a_fresh_process",
            "--nocapture",
        ])
        .env(RESTART_RECEIPT, receipt)
        .output()
        .unwrap();
    assert!(
        child.status.success(),
        "fresh-process recovery failed: {}",
        String::from_utf8_lossy(&child.stderr)
    );
    assert_eq!(journal_count(&f), 3);
    assert_eq!(fs::read(&old_slot).unwrap(), saved_v1);
}

#[test]
fn executable_settings_and_capability_drift_require_review_without_publication() {
    for (file, text, reason) in [
        (
            ".mcp.json",
            r#"{"mcpServers":{"fixture":{"command":"changed-project-command"},"new":{"command":"new-project-command"}}}"#,
            ReviewReason::ExecutableOrSettingsChanged(ProjectSource::ClaudeMcp),
        ),
        (
            ".claude/settings.local.json",
            r#"{"hooks":{"PostToolUse":[]}}"#,
            ReviewReason::ExecutableOrSettingsChanged(ProjectSource::ClaudeLocalSettings),
        ),
        (
            ".claude/skills/unselected/scripts/inert.txt",
            "This fixture cannot run.",
            ReviewReason::CapabilityStructureChanged(ProjectSource::ClaudeSkills),
        ),
        (
            ".claude/skills/ambiguity/SKILL.md",
            SKILL,
            ReviewReason::UnresolvedConflict,
        ),
    ] {
        let f = Fixture::new();
        put(
            &f.scope.root,
            ".claude/skills/unselected/SKILL.md",
            SKILL.replace("name: review", "name: unselected").as_bytes(),
        );
        let drift = DriftFixture::accept(&f);
        let original = fs::read(f.managed.join("project-config.json")).unwrap();
        let revised = revised_instruction("Provide a shorter review.");
        let scope = commit_skill(&f, &revised);
        // Combine a legitimate text edit with another changed source.
        put(&scope.root, file, text.as_bytes());
        let classified = drift.classified(&revised);
        let request = drift_request(&f, UPDATE_ONE, &scope, None, &classified);
        let preview = prepare_project_config_update(&request, &drift.approval()).unwrap();
        assert!(!format!("{preview:?}").contains(CANARY));
        assert_eq!(preview, ProjectConfigUpdatePreview::ReviewRequired(reason));
        assert_eq!(journal_count(&f), 1);
        assert_eq!(
            fs::read(f.managed.join("project-config.json")).unwrap(),
            original
        );
        assert!(!f.private.path().join("project-command-ran").exists());
    }
}

#[test]
fn forged_rule_classification_and_version_binding_do_not_publish() {
    let f = Fixture::new();
    let drift = DriftFixture::accept(&f);
    let text = revised_instruction("Include two sentences.");
    let scope = commit_skill(&f, &text);
    let stale_classification = drift.classified(SKILL);
    let classified = drift.classified(&text);
    let request = drift_request(&f, UPDATE_ONE, &scope, None, &classified);
    let ready_binding = ready(prepare_project_config_update(&request, &drift.approval()).unwrap());
    let stale = drift_request(&f, UPDATE_ONE, &scope, None, &stale_classification);
    assert_eq!(
        apply_project_config_update(&stale, &drift.approval(), &ready_binding)
            .unwrap_err()
            .code,
        ErrorCode::Conflict
    );
    let mut rule = drift.rule.clone();
    rule.revision = "unapproved-rule".into();
    let changed_rule = ConfirmedProjectConfigUpdate {
        rule: &rule,
        ..drift.approval()
    };
    assert_eq!(
        prepare_project_config_update(&request, &changed_rule)
            .unwrap_err()
            .code,
        ErrorCode::PermissionDenied
    );
    let mut binding = drift.binding.clone();
    binding.initial_import.snapshot_sha256 = sha256(b"foreign import");
    let changed_binding = ConfirmedProjectConfigUpdate {
        binding: &binding,
        ..drift.approval()
    };
    assert!(prepare_project_config_update(&request, &changed_binding).is_err());
    let mut foreign = ready_binding;
    foreign.snapshot_sha256 = sha256(b"foreign normalized plan");
    assert_eq!(
        apply_project_config_update(&request, &drift.approval(), &foreign)
            .unwrap_err()
            .code,
        ErrorCode::Conflict
    );
    let mut changed_scope = scope.clone();
    changed_scope.project_revision += 1;
    let changed_request = drift_request(&f, UPDATE_ONE, &changed_scope, None, &classified);
    assert_eq!(
        prepare_project_config_update(&changed_request, &drift.approval()).unwrap(),
        ProjectConfigUpdatePreview::ReviewRequired(ReviewReason::ScopeOrPolicyChanged)
    );
    let mut changed_policy = f.policy.clone();
    changed_policy.provider.version = "unapproved-provider".into();
    let changed_request = ProjectConfigUpdateRequest {
        policy: &changed_policy,
        ..request
    };
    assert_eq!(
        prepare_project_config_update(&changed_request, &drift.approval()).unwrap(),
        ProjectConfigUpdatePreview::ReviewRequired(ReviewReason::ScopeOrPolicyChanged)
    );
    assert_eq!(journal_count(&f), 1);
}

#[test]
fn source_drift_and_destination_replacement_after_preview_cannot_redirect_publication() {
    let f = Fixture::new();
    let drift = DriftFixture::accept(&f);
    let text = revised_instruction("Summarize the report.");
    let scope = commit_skill(&f, &text);
    let classified = drift.classified(&text);
    let request = drift_request(&f, UPDATE_ONE, &scope, None, &classified);
    let expected = ready(prepare_project_config_update(&request, &drift.approval()).unwrap());
    put(
        &scope.root,
        PATH_TO_SKILL,
        revised_instruction("Change the source after preview.").as_bytes(),
    );
    assert_eq!(
        apply_project_config_update(&request, &drift.approval(), &expected)
            .unwrap_err()
            .code,
        ErrorCode::Conflict
    );
    assert_eq!(journal_count(&f), 1);
    put(&scope.root, PATH_TO_SKILL, text.as_bytes());
    let parked = f.private.path().join("parked-project");
    fs::rename(&scope.root, &parked).unwrap();
    fs::create_dir(&scope.root).unwrap();
    put(&scope.root, PATH_TO_SKILL, text.as_bytes());
    assert_eq!(
        prepare_project_config_update(&request, &drift.approval()).unwrap(),
        ProjectConfigUpdatePreview::ReviewRequired(ReviewReason::UnverifiableSource)
    );
    fs::rename(&scope.root, f.private.path().join("foreign-source")).unwrap();
    fs::rename(parked, &scope.root).unwrap();

    let old_target = f.private.path().join("parked-managed");
    fs::rename(&f.managed, &old_target).unwrap();
    fs::create_dir(&f.managed).unwrap();
    fs::set_permissions(&f.managed, fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(
        apply_project_config_update(&request, &drift.approval(), &expected)
            .unwrap_err()
            .code,
        ErrorCode::Conflict
    );
    assert_eq!(fs::read_dir(&f.managed).unwrap().count(), 0);
    assert_eq!(
        fs::read_dir(old_target.join(".baton-transactions"))
            .unwrap()
            .count(),
        1
    );
}

#[test]
fn concurrent_updates_can_create_only_one_successor() {
    let f = Fixture::new();
    let drift = DriftFixture::accept(&f);
    let text = revised_instruction("Keep the summary short.");
    let scope = commit_skill(&f, &text);
    let classified = drift.classified(&text);
    let left = drift_request(&f, UPDATE_ONE, &scope, None, &classified);
    let right = drift_request(&f, UPDATE_TWO, &scope, None, &classified);
    let v1 = ready(prepare_project_config_update(&left, &drift.approval()).unwrap());
    let v2 = ready(prepare_project_config_update(&right, &drift.approval()).unwrap());
    let start = std::sync::Barrier::new(2);
    let results = std::thread::scope(|threads| {
        let l = threads.spawn(|| {
            start.wait();
            apply_project_config_update(&left, &drift.approval(), &v1)
        });
        let r = threads.spawn(|| {
            start.wait();
            apply_project_config_update(&right, &drift.approval(), &v2)
        });
        [l.join().unwrap(), r.join().unwrap()]
    });
    let winners: Vec<_> = results
        .iter()
        .zip([&v1, &v2])
        .filter_map(|(result, version)| {
            result
                .as_ref()
                .is_ok_and(|state| *state == State::Applied)
                .then_some(version)
        })
        .collect();
    assert_eq!(winners.len(), 1, "{results:?}");
    assert_eq!(journal_count(&f), 2);
    let plan = read_project_config_version(&f.managed, &drift.approval(), winners[0])
        .unwrap()
        .unwrap();
    assert_eq!(plan.skills[0].instruction_text(), text);
}
