//! Required-feature genuine18 -> normal Store19 and actual frozen18 backup recovery.
//! No missing-input skip; synthetic/inverse inputs cannot satisfy the fixed seal.
#[path = "fixtures/host18_migration_runtime.rs"]
mod runtime;
use runtime::{
    bounded_read, migrate, private_read, required, sha, write_new, Digests18, Input18, PINNING,
    PROBE, RESTORE,
};
use serde_json::{json, Value};
use std::{
    fs,
    os::unix::fs::{DirBuilderExt, PermissionsExt},
    path::Path,
    process::Command,
};
const FIXTURE_SHA: &str = "613e624582d6437576bbe53fd773be5c880c5d7e2aa424c4fd6931817e77c1f1";
const BUILD_SHA: &str = "84b4d0cf4bb1dfea72ac85c8307593b44ffeecba67609cadf808ea5f90400018";
const DAEMON_SHA: &str = "4fa7bd284899e17ec18f382838a8944ab720bcfd80a4a1319a80801e740cc538";
const CLI_SHA: &str = "255f2c36405b98d447538e3003bb3245ce5f03cefbf0ad99fc8b7992389baabf";
fn controller_program(program: &str) -> String {
    format!(
        "ACCEPTED18 = {}\n{PINNING}\n{program}",
        json!({"build":BUILD_SHA,"batond":DAEMON_SHA,"baton":CLI_SHA})
    )
}
#[test]
fn actual18_normal_store19_backup_is_recoverable_by_exact_frozen18_programs() {
    let fixture = required("AGENTBATON_ACTUAL18_FIXTURE_DIR");
    let frozen = required("AGENTBATON_ACTUAL18_FROZEN_DIR");
    let manifest = required("AGENTBATON_ACTUAL18_BUILD_MANIFEST");
    let interpreter = required("AGENTBATON_ACTUAL18_PYTHON");
    assert_eq!(sha(&private_read(&manifest, 16_777_216)), BUILD_SHA);
    assert_eq!(
        sha(&bounded_read(&frozen.join("batond"), 134_217_728, 0o500)),
        DAEMON_SHA
    );
    assert_eq!(
        sha(&bounded_read(&frozen.join("baton"), 134_217_728, 0o500)),
        CLI_SHA
    );
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let root = tempfile::Builder::new()
        .prefix("ab19-a18-")
        .permissions(fs::Permissions::from_mode(0o700))
        .tempdir_in("/tmp")
        .unwrap()
        .keep();
    let root = fs::canonicalize(root).unwrap();
    let report = migrate(
        Input18 {
            repo,
            fixture: &fixture,
            frozen: &frozen,
            manifest: &manifest,
            interpreter: &interpreter,
            root: &root,
        },
        Digests18 {
            fixture: FIXTURE_SHA,
            build: BUILD_SHA,
            daemon: DAEMON_SHA,
            cli: CLI_SHA,
        },
        controller_program,
    );
    println!("{report}");
}

#[test]
fn actual18_accepted_seals_reject_replacements_before_proof_or_restore_launch() {
    let fixture = required("AGENTBATON_ACTUAL18_FIXTURE_DIR");
    let frozen = required("AGENTBATON_ACTUAL18_FROZEN_DIR");
    let manifest = required("AGENTBATON_ACTUAL18_BUILD_MANIFEST");
    let interpreter = required("AGENTBATON_ACTUAL18_PYTHON");
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let root = tempfile::Builder::new()
        .prefix("ab19-seal-")
        .permissions(fs::Permissions::from_mode(0o700))
        .tempdir_in("/tmp")
        .unwrap()
        .keep();
    let root = fs::canonicalize(root).unwrap();
    let mut cases = vec![];
    for (name, role) in [
        ("manifest", "build"),
        ("daemon", "batond"),
        ("cli", "baton"),
        ("mutually_updated_manifests_and_pair", "build"),
    ] {
        let case = root.join(name);
        fs::DirBuilder::new().mode(0o700).create(&case).unwrap();
        let tools = case.join("frozen");
        fs::DirBuilder::new().mode(0o700).create(&tools).unwrap();
        let fixture_copy = case.join("fixture");
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&fixture_copy)
            .unwrap();
        let mut build: Value =
            serde_json::from_slice(&private_read(&manifest, 16_777_216)).unwrap();
        let mut input: Value =
            serde_json::from_slice(&private_read(&fixture.join("manifest.json"), 16_777_216))
                .unwrap();
        for binary in ["batond", "baton"] {
            let mut raw = bounded_read(&frozen.join(binary), 134_217_728, 0o500);
            if name == "mutually_updated_manifests_and_pair" || role == binary {
                raw.push(0);
            }
            if name == "mutually_updated_manifests_and_pair" {
                // Update all matching digest strings in both manifests together;
                // the externally accepted P0 seal must still reject this pair.
                let old = if binary == "batond" {
                    DAEMON_SHA
                } else {
                    CLI_SHA
                };
                build = serde_json::from_str(
                    &serde_json::to_string(&build)
                        .unwrap()
                        .replace(old, &sha(&raw)),
                )
                .unwrap();
                input = serde_json::from_str(
                    &serde_json::to_string(&input)
                        .unwrap()
                        .replace(old, &sha(&raw)),
                )
                .unwrap();
            }
            let path = tools.join(binary);
            write_new(&path, &raw);
            fs::set_permissions(&path, fs::Permissions::from_mode(0o500)).unwrap();
        }
        let build_path = tools.join("source-build-manifest.json");
        let mut build_bytes = if role == "build" {
            serde_json::to_vec_pretty(&build).unwrap()
        } else {
            private_read(&manifest, 16_777_216)
        };
        if name == "manifest" {
            build_bytes.push(b' ');
        }
        write_new(&build_path, &build_bytes);
        input["tool_provenance_sha256"] = json!(sha(&build_bytes));
        write_new(
            &fixture_copy.join("manifest.json"),
            &serde_json::to_vec_pretty(&input).unwrap(),
        );
        write_new(
            &fixture_copy.join("actual-schema18.sqlite3"),
            &private_read(&fixture.join("actual-schema18.sqlite3"), 16_777_216),
        );
        for (kind, program) in [("proof", PROBE), ("restore", RESTORE)] {
            let runtime = case.join(kind);
            fs::DirBuilder::new().mode(0o700).create(&runtime).unwrap();
            let mut command = Command::new(&interpreter);
            command
                .arg("-c")
                .arg(controller_program(program))
                .env_clear();
            for path in [repo, &fixture_copy, &tools, &build_path] {
                command.arg(path);
            }
            if kind == "restore" {
                command.arg(&runtime);
            }
            let output = command.output().unwrap();
            let mut raw = output.stdout;
            raw.extend_from_slice(&output.stderr);
            write_new(&case.join(format!("{kind}.log")), &raw);
            assert!(!output.status.success());
            assert!(
                String::from_utf8_lossy(&raw).contains(&format!("accepted18 {role} SHA mismatch"))
            );
            assert_eq!(
                fs::read_dir(&runtime).unwrap().count(),
                0,
                "replacement must fail before daemon log/process/session creation"
            );
            cases.push(json!({"case":name,"stage":kind,"exit":output.status.code(),"expected_rejection":format!("accepted18 {role} SHA mismatch"),"log_sha256":sha(&raw),"runtime_empty":true}));
        }
    }
    let report = json!({"status":"passed","dimension":"accepted_P0_tool_seal_replacement_refusal","private_root":root,"cases":cases,"accepted_build":BUILD_SHA,"accepted_daemon":DAEMON_SHA,"accepted_cli":CLI_SHA,"trusted_interpreter":interpreter});
    write_new(
        &root.join("report.json"),
        &serde_json::to_vec_pretty(&report).unwrap(),
    );
    println!("{report}");
}
