//! Linux component tests: genuine Git and opaque owned commands. This is not
//! Native approval, Core lineage/budget authority or a reprepare permission.
#[cfg(target_os = "linux")]
mod linux {
    use agentbaton_workspaces::{
        command_validation::*,
        inspect,
        validation::{
            sha256, ArtifactRequirement, BoundArtifact, ValidationSubject, VersionBinding,
        },
    };
    use std::{
        collections::BTreeMap,
        fs,
        path::{Path, PathBuf},
        process::Command,
        sync::{
            atomic::{AtomicBool, Ordering},
            Arc,
        },
        thread::JoinHandle,
        time::{Duration, Instant},
    };
    const TARGET: &str = "refs/heads/integration";
    fn git(root: &Path, args: &[&str]) -> String {
        let out = Command::new("git")
            .arg("-C")
            .arg(root)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .args([
                "-c",
                "user.name=Component fixture",
                "-c",
                "user.email=fixture@example.invalid",
                "-c",
                "core.hooksPath=/dev/null",
                "-c",
                "commit.gpgsign=false",
            ])
            .args(args)
            .output()
            .unwrap();
        assert!(out.status.success(), "generated Git operation failed");
        String::from_utf8(out.stdout).unwrap().trim().into()
    }
    fn bound(root: &Path, id: &str, path: &str) -> BoundArtifact {
        BoundArtifact::from_approved_root(
            root,
            ArtifactRequirement {
                artifact_id: id.into(),
                relative_path: path.into(),
                max_bytes: 16 * 1024 * 1024,
                expected_sha256: Some(sha256(&fs::read(root.join(path)).unwrap())),
            },
        )
        .unwrap()
    }
    struct Fixture {
        _root: tempfile::TempDir,
        root: PathBuf,
        workspace: PathBuf,
        head: String,
        tree: String,
    }
    struct Running {
        cancel: Arc<AtomicBool>,
        thread: Option<JoinHandle<Result<CommandRun, CommandError>>>,
    }
    impl Running {
        fn finish(mut self) -> CommandRun {
            self.thread.take().unwrap().join().unwrap().unwrap()
        }
    }
    impl Drop for Running {
        fn drop(&mut self) {
            if let Some(t) = self.thread.take() {
                self.cancel.store(true, Ordering::Release);
                if let Ok(Ok(mut run)) = t.join() {
                    let _ = run.stop_and_reap_owned_leader();
                }
            }
        }
    }
    impl Fixture {
        fn new() -> Self {
            use std::os::unix::fs::PermissionsExt;
            let directory = tempfile::tempdir().unwrap();
            let root = directory.path().canonicalize().unwrap();
            fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
            fs::write(root.join("fixture.c"),r#"#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>
int main(int argc,char**argv){if(argc!=4)return 99;FILE*f=fopen(argv[1],"w");if(!f)return 98;fputs("ready\n",f);fclose(f);while(access(argv[2],F_OK))usleep(1000);return atoi(argv[3]);}"#).unwrap();
            let out = Command::new("/usr/bin/cc")
                .env_clear()
                .env("PATH", "/usr/bin:/bin")
                .args(["-O2", "-o"])
                .arg(root.join("fixture"))
                .arg(root.join("fixture.c"))
                .output()
                .unwrap();
            assert!(out.status.success(), "component compiler failed");
            let workspace = root.join("workspace");
            fs::create_dir(&workspace).unwrap();
            git(&workspace, &["init", "--initial-branch=main"]);
            fs::write(workspace.join("input"), b"fixed input").unwrap();
            fs::write(workspace.join("report"), b"fixed report").unwrap();
            fs::create_dir(workspace.join("nested")).unwrap();
            fs::write(workspace.join("nested/unselected"), b"unselected baseline").unwrap();
            git(&workspace, &["add", "input", "report", "nested/unselected"]);
            git(&workspace, &["commit", "-m", "base"]);
            git(&workspace, &["branch", "integration"]);
            let head = git(&workspace, &["rev-parse", "HEAD"]);
            let tree = git(&workspace, &["rev-parse", "HEAD^{tree}"]);
            Self {
                _root: directory,
                root,
                workspace,
                head,
                tree,
            }
        }
        fn begin(&self, exit: i32) -> Running {
            self.begin_expected(exit, vec![0])
        }
        fn begin_expected(&self, exit: i32, expected: Vec<i32>) -> Running {
            let mut input = bound(&self.workspace, "input", "input");
            let mut output = bound(&self.workspace, "report", "report");
            // Data artifacts retain the normal4MiB bound; the native program
            // uses its original16MiB bound. No path/authority is forged by JSON.
            let shrink = |item: &BoundArtifact| {
                let mut v = item.contract_view();
                v.requirement.max_bytes = 1024;
                BoundArtifact::from_verified_contract(&v).unwrap()
            };
            input = shrink(&input);
            output = shrink(&output);
            let rule = ApprovedCommandRule::from_verified_policy(
                "component-owner".into(),
                "component-definition".into(),
                ValidationSubject {
                    task_id: "component-task".into(),
                    attempt_id: "component-attempt".into(),
                    spec_revision: 1,
                    input_revision: self.head.clone(),
                    version: VersionBinding::Git {
                        target_ref: TARGET.into(),
                        expected_target_head: self.head.clone(),
                        source_candidate: self.head.clone(),
                        integration_candidate: self.head.clone(),
                        tree: self.tree.clone(),
                    },
                },
                bound(&self.root, "program", "fixture"),
                inspect(&self.workspace).unwrap(),
                vec![input],
                vec![output],
                CommandSettings {
                    mode: CommandMode::NativeExecutable,
                    arguments: vec![
                        self.root.join("ready").to_string_lossy().into(),
                        self.root.join("release").to_string_lossy().into(),
                        exit.to_string(),
                    ],
                    environment: BTreeMap::new(),
                    expected_exit_codes: expected,
                    wall_time_ms: 1000,
                    cleanup_time_ms: 500,
                    stdout_limit: 128,
                    stderr_limit: 128,
                    require: RequiredEnforcement::default(),
                },
            )
            .unwrap();
            let cancel = Arc::new(AtomicBool::new(false));
            let flag = cancel.clone();
            let deadline = Instant::now() + Duration::from_secs(3);
            let thread = std::thread::spawn(move || {
                run_approved_command_with_deadline(&rule, &flag, deadline)
            });
            Running {
                cancel,
                thread: Some(thread),
            }
        }
        fn ready(&self) {
            let deadline = Instant::now() + Duration::from_secs(2);
            loop {
                if fs::read(self.root.join("ready")).ok().as_deref() == Some(b"ready\n") {
                    return;
                }
                assert!(
                    Instant::now() < deadline,
                    "actual owned command ready barrier"
                );
                std::thread::sleep(Duration::from_millis(1));
            }
        }
        fn advance(&self) -> String {
            let to = git(
                &self.workspace,
                &[
                    "commit-tree",
                    &self.tree,
                    "-p",
                    &self.head,
                    "-m",
                    "descendant",
                ],
            );
            git(&self.workspace, &["update-ref", TARGET, &to, &self.head]);
            to
        }
        fn release(&self) {
            fs::write(self.root.join("release"), b"continue").unwrap();
        }
    }
    #[test]
    fn stopped_target_drift_is_not_pass_and_observation_seals_actual_files() {
        let f = Fixture::new();
        let index = fs::read(f.workspace.join(".git/index")).unwrap();
        let running = f.begin(0);
        f.ready();
        let to = f.advance();
        f.release();
        let run = running.finish();
        assert_eq!(run.view().reason, RunReason::ArtifactContractFailed);
        assert!(!run.view().passed);
        assert_eq!(run.view().exit_code, Some(0));
        assert!(run.view().cleanup.owned_leader_reaped);
        assert!(run.view().cleanup.all_capture_pipes_observed_eof);
        let deadline = Instant::now() + Duration::from_secs(3);
        let observation = run
            .observe_stopped_target_drift(&AtomicBool::new(false), deadline)
            .unwrap()
            .expect("exact target-only change");
        assert_eq!(observation.from_head(), f.head);
        assert_eq!(observation.to_head(), to);
        assert_eq!(observation.target_ref(), TARGET);
        assert_eq!(observation.candidate(), f.head);
        assert_eq!(observation.tree(), f.tree);
        assert_eq!(observation.contract_digest(), run.view().contract_digest);
        observation
            .recheck(&AtomicBool::new(false), deadline)
            .unwrap();
        assert!(!run.view().passed);
        assert!(
            run.verify_current().is_err(),
            "original full strict target guard remains unchanged"
        );
        assert_eq!(fs::read(f.workspace.join(".git/index")).unwrap(), index);
        assert_eq!(fs::read(f.workspace.join("input")).unwrap(), b"fixed input");
        fs::write(f.workspace.join("report"), b"foreign report").unwrap();
        assert!(observation
            .recheck(&AtomicBool::new(false), deadline)
            .is_err());
    }
    #[test]
    fn bad_expected_exit_is_not_hidden_by_target_drift() {
        for change_target in [false, true] {
            let f = Fixture::new();
            let running = f.begin(7);
            f.ready();
            if change_target {
                f.advance();
            }
            f.release();
            let run = running.finish();
            assert!(!run.view().passed);
            assert_eq!(run.view().exit_code, Some(7));
            assert!(run
                .observe_stopped_target_drift(
                    &AtomicBool::new(false),
                    Instant::now() + Duration::from_secs(3)
                )
                .unwrap()
                .is_none());
        }
    }
    #[test]
    fn another_artifact_change_and_cancel_or_expired_probe_never_supply_observation() {
        let f = Fixture::new();
        let running = f.begin(0);
        f.ready();
        f.advance();
        fs::write(f.workspace.join("input"), b"changed input").unwrap();
        f.release();
        let run = running.finish();
        assert!(!run.view().passed);
        assert!(run
            .observe_stopped_target_drift(
                &AtomicBool::new(false),
                Instant::now() + Duration::from_secs(3)
            )
            .unwrap()
            .is_none());
        assert!(run
            .observe_stopped_target_drift(
                &AtomicBool::new(true),
                Instant::now() + Duration::from_secs(3)
            )
            .is_err());
        assert!(run
            .observe_stopped_target_drift(&AtomicBool::new(false), Instant::now())
            .is_err());
    }
    #[test]
    fn an_originally_approved_nonzero_exit_is_not_mistaken_for_unapproved_exit7() {
        let f = Fixture::new();
        let running = f.begin_expected(7, vec![7]);
        f.ready();
        f.advance();
        f.release();
        let run = running.finish();
        assert_eq!(run.view().exit_code, Some(7));
        assert_eq!(run.view().reason, RunReason::ArtifactContractFailed);
        assert!(!run.view().passed);
        assert!(run
            .observe_stopped_target_drift(
                &AtomicBool::new(false),
                Instant::now() + Duration::from_secs(3)
            )
            .unwrap()
            .is_some());
        assert!(
            !run.view().passed,
            "the readonly observation never changes correctness/PASS"
        );
    }
    #[test]
    fn a_raw_tag_slot_is_not_reported_as_its_peeled_commit() {
        let f = Fixture::new();
        git(&f.workspace, &["tag", "-a", "wrapped", "-m", "wrapped"]);
        let tag = git(&f.workspace, &["rev-parse", "refs/tags/wrapped"]);
        let running = f.begin(0);
        f.ready();
        // Corrupt generated private slot, not normal Git/Owner authority.
        fs::write(
            f.workspace.join(".git/refs/heads/integration"),
            format!("{tag}\n"),
        )
        .unwrap();
        f.release();
        let run = running.finish();
        assert_eq!(run.view().reason, RunReason::ArtifactContractFailed);
        assert!(!run.view().passed);
        let error = run
            .observe_stopped_target_drift(
                &AtomicBool::new(false),
                Instant::now() + Duration::from_secs(3),
            )
            .err()
            .expect("tag slot must reject");
        assert_eq!(error, CommandError::CwdChanged);
    }
    #[test]
    fn final_metadata_seal_rejects_unselected_nested_mutations_after_clean_status() {
        for new_file in [false, true] {
            let f = Fixture::new();
            let running = f.begin(0);
            f.ready();
            f.advance();
            f.release();
            let run = running.finish();
            let deadline = Instant::now() + Duration::from_secs(3);
            let observation = run
                .observe_stopped_target_drift(&AtomicBool::new(false), deadline)
                .unwrap()
                .expect("clean candidate before mutation");
            let root_stamp = fs::metadata(&f.workspace).unwrap();
            fs::write(
                f.workspace.join(if new_file {
                    "nested/new-untracked"
                } else {
                    "nested/unselected"
                }),
                b"late mutation",
            )
            .unwrap();
            use std::os::unix::fs::MetadataExt;
            assert_eq!(
                fs::metadata(&f.workspace).unwrap().mtime_nsec(),
                root_stamp.mtime_nsec()
            );
            assert!(observation
                .recheck(&AtomicBool::new(false), deadline)
                .is_err());
            assert!(!run.view().passed);
        }
    }
}
