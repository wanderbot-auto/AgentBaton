//! Generated files only: no native HOME scan, script or Provider execution.
use agentbaton_config::{
    capabilities::native::{
        instruction_only_skill_metadata, InstructionOnlySkill, INSTRUCTION_ONLY_SKILL_SOURCE,
    },
    runtime::*,
    sha256, Ownership,
};
use std::{
    ffi::OsStr,
    fs,
    os::unix::fs::{symlink, FileTypeExt, PermissionsExt},
    path::PathBuf,
};
const TEXT:&str="---\nname: demo-skill\ndescription: Bounded instruction fixture\n---\nRead the selected input.\n";
fn source() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().canonicalize().unwrap().join("demo-skill");
    fs::create_dir(&path).unwrap();
    fs::write(path.join("SKILL.md"), TEXT).unwrap();
    (dir, path)
}
fn plan() -> RuntimePlan {
    let mut p = RuntimePlan::empty(RuntimeProvider::Codex);
    let provenance = SourceProvenance {
        source_ref: INSTRUCTION_ONLY_SKILL_SOURCE.into(),
        revision: "1".into(),
        sha256: sha256(TEXT.as_bytes()),
    };
    p.source_refs.push(provenance.clone());
    p.skills.push(SelectedSkill {
        directory_name: "demo-skill".into(),
        instruction: NonSecretText::approved(TEXT.into()).unwrap(),
        assets: vec![],
        provenance,
    });
    p
}
#[test]
fn strict_source_preserves_exact_bytes_and_detects_mutation_replacement_and_deletion() {
    let (_dir, path) = source();
    let read = InstructionOnlySkill::read_selected(&path).unwrap();
    assert_eq!(read.instruction(), TEXT);
    assert_eq!(read.metadata().name, "demo-skill");
    assert_eq!(read.metadata().sha256, sha256(TEXT.as_bytes()));
    read.verify_unchanged().unwrap();
    fs::write(path.join("SKILL.md"), TEXT.replace("selected", "changed")).unwrap();
    assert!(read.verify_unchanged().is_err());
    fs::write(path.join("SKILL.md"), TEXT).unwrap();
    assert!(read.verify_unchanged().is_err()); // Same bytes do not restore the original change identity.
    let read = InstructionOnlySkill::read_selected(&path).unwrap();
    fs::remove_file(path.join("SKILL.md")).unwrap();
    assert!(read.verify_unchanged().is_err());
    fs::write(path.join("SKILL.md"), TEXT).unwrap();
    let read = InstructionOnlySkill::read_selected(&path).unwrap();
    fs::rename(&path, path.with_file_name("parked")).unwrap();
    assert!(read.verify_unchanged().is_err());
    fs::create_dir(&path).unwrap();
    fs::write(path.join("SKILL.md"), TEXT).unwrap();
    assert!(read.verify_unchanged().is_err());
}
#[test]
fn strict_source_refuses_extras_links_unsafe_ownership_modes_and_nonregular_files() {
    for extra in [
        "scripts",
        "references",
        "assets",
        "agents",
        "agents.yaml",
        ".hidden",
    ] {
        let (_dir, path) = source();
        fs::create_dir(path.join(extra)).unwrap();
        assert!(
            InstructionOnlySkill::read_selected(&path).is_err(),
            "{extra}"
        );
    }
    let (dir, path) = source();
    symlink(&path, dir.path().join("alias")).unwrap();
    assert!(InstructionOnlySkill::read_selected(&dir.path().join("alias")).is_err());
    fs::hard_link(path.join("SKILL.md"), dir.path().join("linked")).unwrap();
    assert!(InstructionOnlySkill::read_selected(&path).is_err());
    fs::remove_file(dir.path().join("linked")).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o777)).unwrap();
    assert!(InstructionOnlySkill::read_selected(&path).is_err());
    fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
    fs::set_permissions(path.join("SKILL.md"), fs::Permissions::from_mode(0o666)).unwrap();
    assert!(InstructionOnlySkill::read_selected(&path).is_err());
    fs::remove_file(path.join("SKILL.md")).unwrap();
    symlink(dir.path().join("linked"), path.join("SKILL.md")).unwrap();
    assert!(InstructionOnlySkill::read_selected(&path).is_err());
    fs::remove_file(path.join("SKILL.md")).unwrap();
    let fifo = path.join("SKILL.md");
    assert!(std::process::Command::new("/usr/bin/mkfifo")
        .args(["-m", "600"])
        .arg(&fifo)
        .status()
        .unwrap()
        .success());
    assert!(fs::symlink_metadata(&fifo).unwrap().file_type().is_fifo());
    assert!(InstructionOnlySkill::read_selected(&path).is_err());
}
#[test]
fn metadata_is_closed_and_bounded_instead_of_discovery_fallback() {
    for text in [
        "---\ndescription: present\n---\nbody",
        "---\nname: demo\n---\nbody",
        "---\nname: demo\nname: second\ndescription: present\n---\nbody",
    ] {
        assert!(instruction_only_skill_metadata("demo-skill", text).is_err());
    }
    for extra in [
        "metadata: {}",
        "allowed-tools: ''",
        "compatibility: codex",
        "dependencies: []",
        "unknown: true",
        "context: dynamic",
    ] {
        let text = TEXT.replacen("description:", &format!("{extra}\ndescription:"), 1);
        assert!(
            instruction_only_skill_metadata("demo-skill", &text).is_err(),
            "{extra}"
        );
    }
    assert!(instruction_only_skill_metadata("../escape", TEXT).is_err());
    assert!(
        instruction_only_skill_metadata("demo-skill", &(TEXT.to_owned() + "!`shell`")).is_err()
    );
    let (_dir, path) = source();
    fs::write(path.join("SKILL.md"), vec![b'x'; 32769]).unwrap();
    assert!(InstructionOnlySkill::read_selected(&path).is_err());
    fs::write(path.join("SKILL.md"), [255, 254]).unwrap();
    assert!(InstructionOnlySkill::read_selected(&path).is_err());
}
#[test]
fn approved_profile_embeds_exact_bytes_in_private_home_and_keeps_legacy_layout() {
    let (dir, path) = source();
    let selected = InstructionOnlySkill::read_selected(&path).unwrap();
    let p = plan();
    assert_eq!(p.skills[0].instruction_text(), selected.instruction());
    drop(dir);
    assert!(selected.verify_unchanged().is_err());
    let runtime = tempfile::tempdir().unwrap();
    fs::set_permissions(runtime.path(), fs::Permissions::from_mode(0o700)).unwrap();
    let root = RuntimeRoot::open(&runtime.path().canonicalize().unwrap()).unwrap();
    let snapshot = root.materialize(p, Ownership::BatonManaged).unwrap();
    let home = PathBuf::from(snapshot.environment().get(OsStr::new("HOME")).unwrap());
    let skill = home.join(".agents/skills/demo-skill/SKILL.md");
    assert_eq!(fs::read(&skill).unwrap(), TEXT.as_bytes());
    assert!(!snapshot
        .provider_configuration_root()
        .join("skills/demo-skill/SKILL.md")
        .exists());
    fs::create_dir(home.join("cache")).unwrap();
    fs::write(home.join("cache/runtime-data"), "mutable").unwrap();
    root.verify_before_launch(&snapshot.descriptor(), RuntimeRequirements::default())
        .unwrap();
    fs::write(&skill, "tampered").unwrap();
    assert!(root
        .verify_before_launch(&snapshot.descriptor(), RuntimeRequirements::default())
        .is_err());
    let mut legacy = plan();
    legacy.skills[0].provenance.source_ref = "old-approved-skill".into();
    legacy.source_refs = vec![];
    let legacy = root.materialize(legacy, Ownership::BatonManaged).unwrap();
    assert_eq!(
        fs::read(
            legacy
                .provider_configuration_root()
                .join("skills/demo-skill/SKILL.md")
        )
        .unwrap(),
        TEXT.as_bytes()
    );
    root.verify_before_launch(&legacy.descriptor(), RuntimeRequirements::default())
        .unwrap();
}
#[test]
fn private_agents_subtree_rejects_unapproved_files_skills_and_links() {
    for kind in ["config", "second-skill", "symlink"] {
        let runtime = tempfile::tempdir().unwrap();
        fs::set_permissions(runtime.path(), fs::Permissions::from_mode(0o700)).unwrap();
        let root = RuntimeRoot::open(&runtime.path().canonicalize().unwrap()).unwrap();
        let snapshot = root.materialize(plan(), Ownership::BatonManaged).unwrap();
        let home = PathBuf::from(snapshot.environment().get(OsStr::new("HOME")).unwrap());
        match kind {
            "config" => fs::write(home.join(".agents/agents.yaml"), "unknown").unwrap(),
            "second-skill" => fs::create_dir(home.join(".agents/skills/other")).unwrap(),
            _ => {
                let skill = home.join(".agents/skills/demo-skill/SKILL.md");
                fs::remove_file(&skill).unwrap();
                symlink("/unapproved", skill).unwrap();
            }
        }
        assert!(
            root.verify_before_launch(&snapshot.descriptor(), RuntimeRequirements::default())
                .is_err(),
            "{kind}"
        );
    }
}
#[test]
fn profile_boundary_rejects_wrong_provider_hash_assets_model_and_metadata() {
    for kind in [
        "provider",
        "hash",
        "revision",
        "asset",
        "model",
        "metadata",
        "missing",
        "duplicate",
    ] {
        let mut p = plan();
        match kind {
            "provider" => p.provider = RuntimeProvider::ClaudeAcp,
            "hash" => p.skills[0].provenance.sha256 = sha256(b"other"),
            "revision" => p.skills[0].provenance.revision = "2".into(),
            "asset" => p.skills[0].assets.push(SkillAsset {
                relative_path: "data".into(),
                content: NonSecretBytes::approved(vec![1]).unwrap(),
                script: false,
            }),
            "model" => p.codex.model = Some(NonSecretText::approved("unapproved".into()).unwrap()),
            "metadata" => {
                let text = TEXT.replace("name:", "unknown: true\nname:");
                p.skills[0].instruction = NonSecretText::approved(text.clone()).unwrap();
                p.skills[0].provenance.sha256 = sha256(text.as_bytes());
                p.source_refs[0] = p.skills[0].provenance.clone();
            }
            "missing" => p.skills.clear(),
            _ => p.skills.push(p.skills[0].clone()),
        }
        assert!(p.validate().is_err(), "{kind}");
    }
}
