//! Explicit same-run genuine18 build, migration, backup and old-binary recovery.
#[path = "fixtures/host18_migration_runtime.rs"]
mod runtime;

use runtime::{
    bounded_read, checked_read, invoke, migrate, required, sha, write_new, Digests18, Identity,
    Input18, PINNING, PROBE, RESTORE,
};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    ffi::OsStr,
    fs,
    os::unix::fs::{DirBuilderExt, PermissionsExt},
    path::{Path, PathBuf},
    process::Command,
};

#[derive(Debug, PartialEq, serde::Serialize)]
struct FileSeal {
    path: PathBuf,
    sha256: String,
    identity: Identity,
}

fn seal_file(path: &Path, max: u64, mode: u32) -> FileSeal {
    assert_eq!(fs::canonicalize(path).unwrap(), path);
    let (raw, identity) = checked_read(path, max, mode);
    FileSeal {
        path: path.to_path_buf(),
        sha256: sha(&raw),
        identity,
    }
}

struct Artifact {
    role: &'static str,
    path: PathBuf,
    limit: u64,
    mode: u32,
}

struct RunPaths {
    root: PathBuf,
    build: PathBuf,
    frozen: PathBuf,
    fixture: PathBuf,
    manifest: PathBuf,
}

impl RunPaths {
    fn new(root: PathBuf) -> Self {
        let build = root.join("build");
        let frozen = build.join("frozen");
        let manifest = frozen.join("source-build-manifest.json");
        let fixture = root.join("fixture");
        Self {
            root,
            build,
            frozen,
            fixture,
            manifest,
        }
    }

    fn build_files(&self) -> [Artifact; 4] {
        [
            artifact("build", self.manifest.clone(), 4_194_304, 0o600),
            artifact("batond", self.frozen.join("batond"), 134_217_728, 0o500),
            artifact("baton", self.frozen.join("baton"), 134_217_728, 0o500),
            artifact("archive", self.build.join("source.tar"), 67_108_864, 0o600),
        ]
    }

    fn fixture_files(&self) -> [Artifact; 4] {
        [
            artifact(
                "fixture",
                self.fixture.join("manifest.json"),
                4_194_304,
                0o600,
            ),
            artifact(
                "database",
                self.fixture.join("actual-schema18.sqlite3"),
                16_777_216,
                0o600,
            ),
            artifact(
                "provenance",
                self.fixture.join("tool-provenance.json"),
                4_194_304,
                0o600,
            ),
            artifact(
                "transcript",
                self.fixture.join("rpc-private.jsonl"),
                1_048_576,
                0o600,
            ),
        ]
    }

    fn artifacts(&self) -> impl Iterator<Item = Artifact> {
        self.build_files().into_iter().chain(self.fixture_files())
    }

    fn controller_args<'a>(&'a self, repo: &'a Path) -> [&'a Path; 4] {
        [repo, &self.fixture, &self.frozen, &self.manifest]
    }
}

fn artifact(role: &'static str, path: PathBuf, limit: u64, mode: u32) -> Artifact {
    Artifact {
        role,
        path,
        limit,
        mode,
    }
}

type Seals = BTreeMap<&'static str, FileSeal>;

fn capture(artifacts: impl IntoIterator<Item = Artifact>) -> Seals {
    artifacts
        .into_iter()
        .map(|file| (file.role, seal_file(&file.path, file.limit, file.mode)))
        .collect()
}

fn fresh_controller(seals: &Seals, program: &str) -> String {
    // The parent supplies these independent in-memory seals, never a fixture's
    // claims about its own bytes. Both positive controllers perform this check.
    format!(
        "FRESH18 = {}\n{PINNING}\n{program}",
        serde_json::to_string(seals).unwrap()
    )
}

fn private_dir(path: &Path) {
    fs::DirBuilder::new().mode(0o700).create(path).unwrap();
}

fn producer(interpreter: &Path, script: &Path, args: &[&OsStr], log_path: &Path) {
    let mut cmd = Command::new(interpreter);
    cmd.arg("-I").arg("-B").arg(script).args(args).env_clear();
    // The reviewed builder constructs a private Cargo home and shares only
    // its registry. The generator ignores the build cache variables.
    for key in [
        "PATH",
        "LANG",
        "LC_ALL",
        "TMPDIR",
        "CARGO_HOME",
        "CARGO_TARGET_DIR",
    ] {
        if let Some(value) = std::env::var_os(key) {
            cmd.env(key, value);
        }
    }
    let output = cmd.output().unwrap();
    let mut log = output.stdout;
    log.extend_from_slice(&output.stderr);
    write_new(log_path, &log);
    assert!(
        output.status.success(),
        "schema18 producer failed; retained {}",
        log_path.display()
    );
}

// Only runs on disposable negative-control copies. Rebind all mutually editable
// provenance fields and show the original verifier accepts that coherent pair.
const REBIND: &str = r#"
import importlib.util,json,sys
from pathlib import Path
repo,fixture,frozen,manifest=map(Path,sys.argv[1:])
spec=importlib.util.spec_from_file_location('control18',repo/'scripts/generate-schema18-migration-fixture.py')
g=importlib.util.module_from_spec(spec);spec.loader.exec_module(g)
h=g.H;tools=h.tool_records(frozen)
build=h.parse_json(manifest.read_bytes())
build.update(frozen_directory=str(frozen),binaries=tools)
raw=h.canonical(build)
manifest.write_bytes(raw)
(fixture/'tool-provenance.json').write_bytes(raw)
path=fixture/'manifest.json'
record=h.parse_json(path.read_bytes())
record.update(tool_provenance_sha256=h.sha(raw),tool_metadata=tools,tool_metadata_after=tools,
              binaries={name:value['sha256'] for name,value in tools.items()})
path.write_bytes(h.canonical(record))
g.verify_fixture(fixture,frozen,manifest)
print(json.dumps({'verified_coherent_fixture_and_build':True}))
"#;

fn replace_copy(file: &Artifact, mut bytes: Vec<u8>, change_bytes: bool) {
    if change_bytes {
        bytes.push(b' ');
    }
    let replacement = file.path.with_extension("replacement");
    write_new(&replacement, &bytes);
    fs::set_permissions(&replacement, fs::Permissions::from_mode(file.mode)).unwrap();
    fs::rename(replacement, &file.path).unwrap();
}

fn refusal_controls(
    original: &RunPaths,
    original_seals: &Seals,
    repo: &Path,
    interpreter: &Path,
) -> Vec<Value> {
    let mut observations = Vec::new();
    for (name, role, reason) in [
        ("modified_build", "build", "SHA mismatch"),
        ("modified_daemon", "batond", "SHA mismatch"),
        ("modified_cli", "baton", "SHA mismatch"),
        ("modified_fixture", "fixture", "SHA mismatch"),
        ("modified_db", "database", "SHA mismatch"),
        ("same_bytes_replaced_inode", "build", "identity mismatch"),
        (
            "coherent_changed_manifests_and_pair",
            "build",
            "SHA mismatch",
        ),
    ] {
        let copy = RunPaths::new(original.root.join(name));
        for directory in [&copy.root, &copy.build, &copy.frozen, &copy.fixture] {
            private_dir(directory);
        }
        for (src, dst) in original.artifacts().zip(copy.artifacts()) {
            let raw = bounded_read(&src.path, src.limit, src.mode);
            assert_eq!(sha(&raw), original_seals[src.role].sha256);
            write_new(&dst.path, &raw);
            fs::set_permissions(&dst.path, fs::Permissions::from_mode(dst.mode)).unwrap();
        }
        let args = copy.controller_args(repo);
        let rebound = runtime::python(
            interpreter,
            REBIND,
            &args,
            &copy.root.join("coherent-baseline.log"),
        );
        assert_eq!(rebound["verified_coherent_fixture_and_build"], true);
        let sealed = capture(copy.artifacts());
        if name == "coherent_changed_manifests_and_pair" {
            for file in copy
                .build_files()
                .into_iter()
                .filter(|file| file.mode == 0o500)
            {
                replace_copy(&file, bounded_read(&file.path, file.limit, file.mode), true);
            }
            let rebound = runtime::python(
                interpreter,
                REBIND,
                &args,
                &copy.root.join("coherent-replacement.log"),
            );
            assert_eq!(rebound["verified_coherent_fixture_and_build"], true);
        } else {
            let file = copy.artifacts().find(|file| file.role == role).unwrap();
            replace_copy(
                &file,
                bounded_read(&file.path, file.limit, file.mode),
                reason == "SHA mismatch",
            );
        }
        for (stage, program) in [("proof", PROBE), ("restore", RESTORE)] {
            let runtime = copy.root.join(stage);
            private_dir(&runtime);
            let args = args
                .into_iter()
                .chain((stage == "restore").then_some(runtime.as_path()))
                .collect::<Vec<_>>();
            let output = invoke(interpreter, &fresh_controller(&sealed, program), &args);
            let expected = format!("accepted18 {role} {reason}");
            let log = copy.root.join(format!("{stage}.log"));
            let mut raw = output.stdout;
            raw.extend_from_slice(&output.stderr);
            write_new(&log, &raw);
            assert!(
                !output.status.success() && String::from_utf8_lossy(&raw).contains(&expected),
                "wrong refusal: {}",
                log.display()
            );
            assert_eq!(fs::read_dir(&runtime).unwrap().count(), 0);
            observations.push(json!({"case":name,"stage":stage,"exit":output.status.code(),
                "expected_rejection":expected,"private_log":log,"log_sha256":sha(&raw),"runtime_empty":true}));
        }
    }
    observations
}

#[test]
fn fresh18_trusted_build_migrates_and_restores_before_accepting() {
    let interpreter = required("AGENTBATON_FRESH18_PYTHON");
    let cargo = required("AGENTBATON_FRESH18_CARGO");
    let rustc = required("AGENTBATON_FRESH18_RUSTC");
    let target_root = required("CARGO_TARGET_DIR");
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let root = tempfile::Builder::new()
        .prefix("ab19-f18-")
        .permissions(fs::Permissions::from_mode(0o700))
        .tempdir_in("/tmp")
        .unwrap()
        .keep();
    let root = fs::canonicalize(root).unwrap();
    let paths = RunPaths::new(root);
    let target = target_root.join(paths.root.file_name().unwrap());
    producer(
        &interpreter,
        &repo.join("scripts/prepare-schema18-migration-fixture.py"),
        &[
            OsStr::new("--repository"),
            repo.as_os_str(),
            OsStr::new("--output"),
            paths.build.as_os_str(),
            OsStr::new("--target-dir"),
            target.as_os_str(),
            OsStr::new("--cargo"),
            cargo.as_os_str(),
            OsStr::new("--rustc"),
            rustc.as_os_str(),
        ],
        &paths.root.join("trusted-build.log"),
    );
    let mut seals = capture(paths.build_files());
    producer(
        &interpreter,
        &repo.join("scripts/generate-schema18-migration-fixture.py"),
        &[
            OsStr::new("--frozen"),
            paths.frozen.as_os_str(),
            OsStr::new("--build-manifest"),
            paths.manifest.as_os_str(),
            OsStr::new("--output"),
            paths.fixture.as_os_str(),
        ],
        &paths.root.join("trusted-generator.log"),
    );
    assert_eq!(
        capture(paths.build_files()),
        seals,
        "build changed during fixture generation"
    );
    seals.extend(capture(paths.fixture_files()));
    let working = paths.root.join("migration");
    private_dir(&working);
    let report = migrate(
        Input18 {
            repo,
            fixture: &paths.fixture,
            frozen: &paths.frozen,
            manifest: &paths.manifest,
            interpreter: &interpreter,
            root: &working,
        },
        Digests18 {
            fixture: &seals["database"].sha256,
            build: &seals["build"].sha256,
            daemon: &seals["batond"].sha256,
            cli: &seals["baton"].sha256,
        },
        |program| fresh_controller(&seals, program),
    );
    let refusals = refusal_controls(&paths, &seals, repo, &interpreter);
    assert_eq!(
        capture(paths.artifacts()),
        seals,
        "original trusted artifact changed"
    );
    let result = json!({"status":"passed","migration":report,"in_run_seals":seals,
        "refusals":refusals,"private_root":paths.root,"producer_target":target});
    write_new(
        &paths.root.join("report.json"),
        &serde_json::to_vec_pretty(&result).unwrap(),
    );
    println!("{result}");
}
