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
        os::unix::fs::PermissionsExt,
        path::{Path, PathBuf},
        process::Command,
        sync::{
            atomic::{AtomicBool, Ordering},
            Arc, OnceLock,
        },
        time::{Duration, Instant},
    };
    struct CompilerFixture {
        _directory: tempfile::TempDir,
        executable: PathBuf,
    }
    fn compiled() -> &'static CompilerFixture {
        static FIXTURE: OnceLock<CompilerFixture> = OnceLock::new();
        FIXTURE.get_or_init(||{
            let directory=tempfile::tempdir().unwrap();let source=directory.path().join("fixture.c");let executable=directory.path().join("fixture");
            fs::write(&source,r#"
#include <unistd.h>
#include <fcntl.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <signal.h>
#include <sys/types.h>
#include <sys/stat.h>
static void put(const char *path,const char *text){int f=open(path,O_CREAT|O_TRUNC|O_WRONLY,0600);if(f<0)exit(91);write(f,text,strlen(text));close(f);}
int main(int argc,char **argv){setvbuf(stdout,NULL,_IONBF,0);setvbuf(stderr,NULL,_IONBF,0);const char *mode=argc>1?argv[1]:"normal";put("started","yes");
if(!strcmp(mode,"env")){if(getenv("PATH")||getenv("HOME")||!getenv("APPROVED")||strcmp(getenv("APPROVED"),"literal"))return 71;put("out","ok");printf("clean");return 0;}
if(!strcmp(mode,"replace")){put(argv[2],"changed validator");put("out","ok");printf("approved-snapshot");return 0;}
if(!strcmp(mode,"input")){put("input","changed");put("out","ok");return 0;}
if(!strcmp(mode,"restore")){put("input","temporarily changed");put("input","input snapshot");put("out","ok");return 0;}
put("out","ok");
if(!strcmp(mode,"loop")){for(;;)usleep(1000);}
if(!strcmp(mode,"stdout")){for(int i=0;i<100000;i++)printf("0123456789");return 0;}
if(!strcmp(mode,"stderr")){for(int i=0;i<100000;i++)fprintf(stderr,"0123456789");return 0;}
if(!strcmp(mode,"forkpipe")){int ack[2];pipe(ack);if(!fork()){close(ack[0]);write(ack[1],"x",1);close(ack[1]);usleep(700000);put("late-child","must-not-happen");_exit(0);}close(ack[1]);char c;read(ack[0],&c,1);close(ack[0]);return 0;}
if(!strcmp(mode,"escape")){int ack[2];pipe(ack);if(!fork()){close(ack[0]);setsid();write(ack[1],"x",1);close(ack[1]);for(int i=0;i<1000 && access("release-escaped",F_OK);i++)usleep(1000);put("escaped-finished","bounded-fixture");_exit(0);}close(ack[1]);char c;read(ack[0],&c,1);close(ack[0]);return 0;}
printf("ready");fprintf(stderr,"diag");if(!strcmp(mode,"exit7"))return 7;if(!strcmp(mode,"signal"))raise(SIGTERM);return 0;}
"#).unwrap();
            let result=Command::new("/usr/bin/cc").env_clear().env("PATH","/usr/bin:/bin").args(["-O2","-o"]).arg(&executable).arg(&source).output().unwrap();assert!(result.status.success(),"{}",String::from_utf8_lossy(&result.stderr));
            CompilerFixture {_directory:directory,executable}
        })
    }
    struct Fixture {
        _directory: tempfile::TempDir,
        root: PathBuf,
        tool_root: PathBuf,
        program: PathBuf,
    }
    impl Fixture {
        fn new() -> Self {
            let directory = tempfile::tempdir().unwrap();
            let base = directory.path().canonicalize().unwrap();
            let root = base.join("cwd");
            let tool_root = base.join("tools");
            fs::create_dir(&root).unwrap();
            fs::create_dir(&tool_root).unwrap();
            let program = tool_root.join("validator");
            fs::copy(&compiled().executable, &program).unwrap();
            fs::set_permissions(&program, fs::Permissions::from_mode(0o700)).unwrap();
            fs::write(root.join("input"), b"input snapshot").unwrap();
            Self {
                _directory: directory,
                root,
                tool_root,
                program,
            }
        }
        fn artifact(
            &self,
            id: &str,
            root: &Path,
            path: &str,
            max: u64,
            hash: Option<String>,
        ) -> BoundArtifact {
            BoundArtifact::from_approved_root(
                root,
                ArtifactRequirement {
                    artifact_id: id.into(),
                    relative_path: path.into(),
                    max_bytes: max,
                    expected_sha256: hash,
                },
            )
            .unwrap()
        }
        fn settings(&self, mode: &str) -> CommandSettings {
            CommandSettings {
                mode: CommandMode::NativeExecutable,
                arguments: vec![mode.into()],
                environment: BTreeMap::new(),
                expected_exit_codes: vec![0],
                wall_time_ms: 1000,
                cleanup_time_ms: 100,
                stdout_limit: 4096,
                stderr_limit: 4096,
                require: RequiredEnforcement {
                    sealed_executable_image: true,
                    ..Default::default()
                },
            }
        }
        fn rule(&self, settings: CommandSettings) -> ApprovedCommandRule {
            self.try_rule(settings).unwrap()
        }
        fn try_rule(&self, settings: CommandSettings) -> Result<ApprovedCommandRule, CommandError> {
            self.try_rule_for_subject(
                settings,
                ValidationSubject {
                    task_id: "task".into(),
                    attempt_id: "attempt".into(),
                    spec_revision: 1,
                    input_revision: "input-v1".into(),
                    version: VersionBinding::Data {
                        dataset_revision: "dataset-v1".into(),
                    },
                },
            )
        }
        fn try_rule_for_subject(
            &self,
            settings: CommandSettings,
            subject: ValidationSubject,
        ) -> Result<ApprovedCommandRule, CommandError> {
            ApprovedCommandRule::from_verified_policy(
                "owner-1".into(),
                "policy-1".into(),
                subject,
                self.artifact(
                    "validator-program",
                    &self.tool_root,
                    "validator",
                    16 * 1024 * 1024,
                    Some(sha256(&fs::read(&self.program).unwrap())),
                ),
                inspect(&self.root).unwrap(),
                vec![self.artifact(
                    "input",
                    &self.root,
                    "input",
                    4096,
                    Some(sha256(b"input snapshot")),
                )],
                vec![self.artifact("output", &self.root, "out", 4096, Some(sha256(b"ok")))],
                settings,
            )
        }
    }
    #[test]
    fn real_fixed_native_command_captures_separate_streams_and_only_accepts_command_scope() {
        let f = Fixture::new();
        let rule = f.rule(f.settings("normal"));
        let run = run_approved_command(&rule, &AtomicBool::new(false)).unwrap();
        assert!(run.view().passed);
        assert_eq!(run.captured_stdout(), b"ready");
        assert_eq!(run.captured_stderr(), b"diag");
        assert_eq!(run.view().main_image, "linux_sealed_elf_snapshot");
        assert_eq!(run.view().scope, SCOPE);
        assert_eq!(run.view().task_acceptance, "not_established");
        assert!(!run.view().integration_authorized);
        assert!(run.view().cleanup.owned_leader_reaped);
        assert_eq!(
            run.view().cleanup.descendant_containment,
            "not_enforced_unknown"
        );
        assert_eq!(
            run.accept_command(&rule).unwrap().validation_digest(),
            run.view().digest
        );
    }
    #[test]
    fn exact_expected_nonzero_exit_is_distinct_from_signal_or_unexpected_exit() {
        let f = Fixture::new();
        let rule = f.rule(f.settings("exit7"));
        let run = run_approved_command(&rule, &AtomicBool::new(false)).unwrap();
        assert!(!run.view().passed);
        assert_eq!(run.view().exit_code, Some(7));
        let mut expected = f.settings("exit7");
        expected.expected_exit_codes = vec![7];
        let rule = f.rule(expected);
        assert!(
            run_approved_command(&rule, &AtomicBool::new(false))
                .unwrap()
                .view()
                .passed
        );
        let run =
            run_approved_command(&f.rule(f.settings("signal")), &AtomicBool::new(false)).unwrap();
        assert!(!run.view().passed);
        assert_eq!(run.view().exit_signal, Some(libc::SIGTERM));
    }
    #[test]
    fn changed_program_and_inputs_are_rejected_before_start() {
        let f = Fixture::new();
        let rule = f.rule(f.settings("normal"));
        fs::write(&f.program, b"not the approved image").unwrap();
        assert!(matches!(
            run_approved_command(&rule, &AtomicBool::new(false)),
            Err(CommandError::ProgramChanged)
        ));
        assert!(!f.root.join("started").exists());
        let f = Fixture::new();
        let rule = f.rule(f.settings("normal"));
        fs::write(f.root.join("input"), "changed").unwrap();
        assert!(matches!(
            run_approved_command(&rule, &AtomicBool::new(false)),
            Err(CommandError::InputChanged)
        ));
        assert!(!f.root.join("started").exists());
    }
    #[test]
    fn sealed_image_runs_approved_bytes_even_if_original_path_is_replaced() {
        let f = Fixture::new();
        let mut settings = f.settings("replace");
        settings
            .arguments
            .push(f.program.to_string_lossy().into_owned());
        let rule = f.rule(settings);
        let run = run_approved_command(&rule, &AtomicBool::new(false)).unwrap();
        assert_eq!(run.captured_stdout(), b"approved-snapshot");
        assert_eq!(fs::read(&f.program).unwrap(), b"changed validator");
        assert!(!run.view().passed);
        assert_eq!(run.view().reason, RunReason::ArtifactContractFailed);
    }
    #[test]
    fn required_unknown_enforcement_is_rejected_before_any_program_start() {
        for field in 0..5 {
            let f = Fixture::new();
            let mut s = f.settings("normal");
            match field {
                0 => s.require.filesystem_isolation = true,
                1 => s.require.network_isolation = true,
                2 => s.require.process_tree_containment = true,
                3 => s.require.execution_time_input_immutability = true,
                4 => s.require.auxiliary_source_immutability = true,
                _ => unreachable!(),
            };
            assert!(matches!(
                run_approved_command(&f.rule(s), &AtomicBool::new(false)),
                Err(CommandError::EnforcementUnavailable)
            ));
            assert!(!f.root.join("started").exists());
        }
    }
    #[test]
    fn cancellation_before_and_during_run_never_claims_full_tree_stopped() {
        let f = Fixture::new();
        let run =
            run_approved_command(&f.rule(f.settings("normal")), &AtomicBool::new(true)).unwrap();
        assert!(!run.view().process_started);
        assert_eq!(run.view().reason, RunReason::CancelledBeforeSpawn);
        assert!(!f.root.join("started").exists());
        let rule = f.rule(f.settings("loop"));
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = cancel.clone();
        let thread =
            std::thread::spawn(move || run_approved_command(&rule, &worker_cancel).unwrap());
        let deadline = Instant::now() + Duration::from_secs(2);
        while !f.root.join("started").exists() {
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(2));
        }
        cancel.store(true, Ordering::Relaxed);
        let run = thread.join().unwrap();
        assert_eq!(run.view().reason, RunReason::CancelRequested);
        assert!(!run.view().passed);
        assert!(run.view().cleanup.owned_leader_reaped);
        assert_eq!(
            run.view().cleanup.descendant_containment,
            "not_enforced_unknown"
        );
    }
    #[test]
    fn timeout_and_each_stream_limit_are_actual_failures_with_bounded_capture() {
        let f = Fixture::new();
        let mut s = f.settings("loop");
        s.wall_time_ms = 30;
        let start = Instant::now();
        let run = run_approved_command(&f.rule(s), &AtomicBool::new(false)).unwrap();
        assert_eq!(run.view().reason, RunReason::DeadlineExceeded);
        assert!(run.view().cleanup.owned_leader_reaped);
        assert!(start.elapsed() < Duration::from_secs(1));
        for mode in ["stdout", "stderr"] {
            let f = Fixture::new();
            let mut s = f.settings(mode);
            s.stdout_limit = 128;
            s.stderr_limit = 128;
            let run = run_approved_command(&f.rule(s), &AtomicBool::new(false)).unwrap();
            assert_eq!(run.view().reason, RunReason::CaptureLimit);
            assert!(!run.view().passed);
            assert!(run.captured_stdout().len() <= 128 && run.captured_stderr().len() <= 128);
            assert!(run.view().stdout.truncated || run.view().stderr.truncated);
        }
    }
    #[test]
    fn owned_inherited_pipe_child_is_killed_before_original_leader_is_reaped() {
        let f = Fixture::new();
        let start = Instant::now();
        let run =
            run_approved_command(&f.rule(f.settings("forkpipe")), &AtomicBool::new(false)).unwrap();
        assert!(run.view().cleanup.owned_leader_reaped);
        assert!(run.view().cleanup.owned_group_signal_sent);
        assert!(run.view().cleanup.all_capture_pipes_observed_eof);
        assert!(start.elapsed() < Duration::from_secs(1));
        std::thread::sleep(Duration::from_millis(750));
        assert!(!f.root.join("late-child").exists());
    }
    #[test]
    fn escaped_fixture_pipe_is_bounded_and_containment_remains_unknown() {
        let f = Fixture::new();
        let mut s = f.settings("escape");
        s.cleanup_time_ms = 30;
        let start = Instant::now();
        let run = run_approved_command(&f.rule(s), &AtomicBool::new(false)).unwrap();
        assert_eq!(run.view().reason, RunReason::CleanupUnknown);
        assert!(!run.view().passed);
        assert!(run.view().cleanup.owned_leader_reaped);
        assert!(!run.view().cleanup.all_capture_pipes_observed_eof);
        assert!(start.elapsed() < Duration::from_secs(1));
        fs::write(f.root.join("release-escaped"), "release").unwrap();
        let deadline = Instant::now() + Duration::from_secs(1);
        while !f.root.join("escaped-finished").exists() {
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    #[test]
    fn environment_is_not_inherited_and_values_are_not_in_contract_logs() {
        let f = Fixture::new();
        let mut s = f.settings("env");
        s.environment.insert("APPROVED".into(), "literal".into());
        let rule = f.rule(s);
        let run = run_approved_command(&rule, &AtomicBool::new(false)).unwrap();
        assert!(run.view().passed);
        assert_eq!(run.captured_stdout(), b"clean");
        assert!(!serde_json::to_string(&rule.contract_view())
            .unwrap()
            .contains("literal"));
        let mut bad = f.settings("normal");
        bad.environment
            .insert("LD_PRELOAD".into(), "unapproved.so".into());
        assert!(f.try_rule(bad).is_err());
    }
    #[test]
    fn cwd_replacement_and_post_run_input_drift_invalidate_evidence() {
        let f = Fixture::new();
        let rule = f.rule(f.settings("normal"));
        let moved = f.root.with_file_name("old-cwd");
        fs::rename(&f.root, moved).unwrap();
        fs::create_dir(&f.root).unwrap();
        assert!(matches!(
            run_approved_command(&rule, &AtomicBool::new(false)),
            Err(CommandError::CwdChanged)
        ));
        let f = Fixture::new();
        let rule = f.rule(f.settings("input"));
        let run = run_approved_command(&rule, &AtomicBool::new(false)).unwrap();
        assert_eq!(run.view().reason, RunReason::ArtifactContractFailed);
        assert!(!run.view().passed);
    }
    #[test]
    fn limits_and_program_path_are_in_the_complete_contract_digest() {
        let f = Fixture::new();
        let first = f.rule(f.settings("normal"));
        let mut changed = f.settings("normal");
        changed.stdout_limit = 123;
        let second = f.rule(changed);
        assert_ne!(first.contract_digest(), second.contract_digest());
        let mut invalid = f.settings("normal");
        invalid.stdout_limit = 65537;
        assert!(f.try_rule(invalid).is_err());
    }
    #[test]
    fn git_candidate_target_and_root_bindings_are_checked_before_and_after_command() {
        let f = Fixture::new();
        let git = |args: &[&str]| {
            let out = Command::new("/usr/bin/git")
                .env_clear()
                .env("PATH", "/usr/bin:/bin")
                .env("GIT_CONFIG_NOSYSTEM", "1")
                .env("GIT_CONFIG_GLOBAL", "/dev/null")
                .arg("-C")
                .arg(&f.root)
                .args(args)
                .output()
                .unwrap();
            assert!(
                out.status.success(),
                "{}",
                String::from_utf8_lossy(&out.stderr)
            );
            String::from_utf8(out.stdout).unwrap().trim().to_owned()
        };
        git(&["init", "-b", "main"]);
        git(&["config", "user.name", "fixture"]);
        git(&["config", "user.email", "fixture@invalid"]);
        git(&["config", "core.hooksPath", "/dev/null"]);
        fs::write(f.root.join(".gitignore"), "out\nstarted\n").unwrap();
        git(&["add", "input", ".gitignore"]);
        git(&["commit", "-m", "generated fixture"]);
        let head = git(&["rev-parse", "HEAD"]);
        let tree = git(&["rev-parse", "HEAD^{tree}"]);
        let subject = ValidationSubject {
            task_id: "task".into(),
            attempt_id: "attempt".into(),
            spec_revision: 1,
            input_revision: "input-v1".into(),
            version: VersionBinding::Git {
                target_ref: "refs/heads/main".into(),
                expected_target_head: head.clone(),
                source_candidate: head.clone(),
                integration_candidate: head.clone(),
                tree,
            },
        };
        let rule = f
            .try_rule_for_subject(f.settings("normal"), subject.clone())
            .unwrap();
        let run = run_approved_command(&rule, &AtomicBool::new(false)).unwrap();
        assert!(run.view().passed);
        assert!(run.accept_command(&rule).is_ok());
        git(&["commit", "--allow-empty", "-m", "target changed"]);
        assert!(run.accept_command(&rule).is_err());
        assert!(matches!(
            run_approved_command(&rule, &AtomicBool::new(false)),
            Err(CommandError::CwdChanged)
        ));
        let mut different_subject = subject;
        different_subject.attempt_id = "other-attempt".into();
        let different = f
            .try_rule_for_subject(f.settings("normal"), different_subject)
            .unwrap();
        assert_ne!(rule.contract_digest(), different.contract_digest());
    }
    #[test]
    fn native_mode_rejects_shebang_and_program_root_replacement_without_start() {
        let f = Fixture::new();
        fs::write(&f.program, b"#!/bin/sh\nprintf unsafe > started\n").unwrap();
        let rule = f.rule(f.settings("normal"));
        assert!(matches!(
            run_approved_command(&rule, &AtomicBool::new(false)),
            Err(CommandError::UnsupportedProgram)
        ));
        assert!(!f.root.join("started").exists());
        let f = Fixture::new();
        let rule = f.rule(f.settings("normal"));
        let old = f.tool_root.with_file_name("old-tools");
        fs::rename(&f.tool_root, &old).unwrap();
        fs::create_dir(&f.tool_root).unwrap();
        fs::copy(old.join("validator"), &f.program).unwrap();
        assert!(matches!(
            run_approved_command(&rule, &AtomicBool::new(false)),
            Err(CommandError::ProgramChanged)
        ));
        assert!(!f.root.join("started").exists());
    }
    #[test]
    fn restored_input_is_only_a_before_after_observation_not_continuous_immutability() {
        let f = Fixture::new();
        let run =
            run_approved_command(&f.rule(f.settings("restore")), &AtomicBool::new(false)).unwrap();
        assert!(run.view().passed);
        assert_eq!(fs::read(f.root.join("input")).unwrap(), b"input snapshot");
        assert_eq!(
            run.view().cleanup.input_immutability,
            "before_after_observations_only"
        );
        assert!(
            !run.view()
                .contract
                .require
                .execution_time_input_immutability
        );
        assert_eq!(run.view().task_acceptance, "not_established");
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
mod scripts {
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
        sync::atomic::AtomicBool,
    };
    struct Fixture {
        _directory: tempfile::TempDir,
        root: PathBuf,
        shell: PathBuf,
    }
    impl Fixture {
        fn new(script: &str) -> Self {
            let directory = tempfile::tempdir().unwrap();
            let root = directory.path().canonicalize().unwrap();
            fs::write(root.join("input"), b"input").unwrap();
            fs::write(root.join("approved-script"), script).unwrap();
            Self {
                _directory: directory,
                root,
                shell: Path::new("/bin/sh").canonicalize().unwrap(),
            }
        }
        fn artifact(
            &self,
            id: &str,
            root: &Path,
            path: &str,
            max: u64,
            hash: Option<String>,
        ) -> BoundArtifact {
            BoundArtifact::from_approved_root(
                root,
                ArtifactRequirement {
                    artifact_id: id.into(),
                    relative_path: path.into(),
                    max_bytes: max,
                    expected_sha256: hash,
                },
            )
            .unwrap()
        }
        fn settings(&self) -> CommandSettings {
            CommandSettings {
                mode: CommandMode::PosixShStdin {
                    script: self.artifact(
                        "script",
                        &self.root,
                        "approved-script",
                        65_536,
                        Some(sha256(
                            &fs::read(self.root.join("approved-script")).unwrap(),
                        )),
                    ),
                },
                arguments: vec!["ok".into()],
                environment: BTreeMap::new(),
                expected_exit_codes: vec![0],
                wall_time_ms: 1000,
                cleanup_time_ms: 100,
                stdout_limit: 4096,
                stderr_limit: 4096,
                require: RequiredEnforcement::default(),
            }
        }
        fn rule(&self, settings: CommandSettings) -> ApprovedCommandRule {
            ApprovedCommandRule::from_verified_policy(
                "owner-script".into(),
                "policy-script-1".into(),
                ValidationSubject {
                    task_id: "task".into(),
                    attempt_id: "attempt".into(),
                    spec_revision: 1,
                    input_revision: "input-v1".into(),
                    version: VersionBinding::Data {
                        dataset_revision: "dataset-v1".into(),
                    },
                },
                self.artifact(
                    "program",
                    self.shell.parent().unwrap(),
                    self.shell.file_name().unwrap().to_str().unwrap(),
                    16 * 1024 * 1024,
                    Some(sha256(&fs::read(&self.shell).unwrap())),
                ),
                inspect(&self.root).unwrap(),
                vec![self.artifact("input", &self.root, "input", 4096, Some(sha256(b"input")))],
                vec![self.artifact("out", &self.root, "out", 4096, Some(sha256(b"ok")))],
                settings,
            )
            .unwrap()
        }
    }
    #[test]
    fn approved_script_bytes_use_fixed_sh_stdin_and_literal_arguments() {
        let f = Fixture::new("printf '%s' \"$1\" > out\nprintf '%s' \"$2\"\nprintf diag >&2\n");
        let mut settings = f.settings();
        settings.arguments.push("$(touch injected); literal".into());
        let rule = f.rule(settings);
        let run = run_approved_command(&rule, &AtomicBool::new(false)).unwrap();
        assert!(run.view().passed);
        assert_eq!(run.captured_stdout(), b"$(touch injected); literal");
        assert_eq!(run.captured_stderr(), b"diag");
        assert!(!f.root.join("injected").exists());
        assert_eq!(&rule.contract_view().arguments[..2], &["-s", "--"]);
        assert!(run.accept_command(&rule).is_ok());
        #[cfg(target_os = "macos")]
        {
            assert_eq!(run.view().main_image, "macos_root_protected_mach_o_path");
            assert!(run.view().protected_program.is_some());
        }
    }
    #[test]
    fn changed_script_is_rejected_without_running_any_script_content() {
        let f = Fixture::new("printf ok > out\n");
        let rule = f.rule(f.settings());
        fs::write(f.root.join("approved-script"), "printf changed > started\n").unwrap();
        assert!(matches!(
            run_approved_command(&rule, &AtomicBool::new(false)),
            Err(CommandError::ScriptChanged)
        ));
        assert!(!f.root.join("started").exists());
        assert!(!f.root.join("out").exists());
    }
    #[test]
    fn script_path_mutation_does_not_change_executed_memory_bytes_and_fails_integrity() {
        let f = Fixture::new("printf 'different script' > \"$2\"\nprintf '%s' \"$1\" > out\nprintf approved-memory\n");
        let mut settings = f.settings();
        settings
            .arguments
            .push(f.root.join("approved-script").to_str().unwrap().into());
        let rule = f.rule(settings);
        let run = run_approved_command(&rule, &AtomicBool::new(false)).unwrap();
        assert_eq!(run.captured_stdout(), b"approved-memory");
        assert_eq!(
            fs::read(f.root.join("approved-script")).unwrap(),
            b"different script"
        );
        assert!(!run.view().passed);
        assert_eq!(run.view().reason, RunReason::ArtifactContractFailed);
        assert!(run.accept_command(&rule).is_err());
    }
    #[test]
    fn full_subject_rule_and_output_changes_invalidate_command_acceptance() {
        let f = Fixture::new("printf ok > out\n");
        let rule = f.rule(f.settings());
        let run = run_approved_command(&rule, &AtomicBool::new(false)).unwrap();
        assert!(run.view().passed);
        let mut changed = f.settings();
        changed.wall_time_ms = 999;
        let changed = f.rule(changed);
        assert!(run.accept_command(&changed).is_err());
        fs::write(f.root.join("out"), "changed").unwrap();
        assert!(run.verify_current().is_err());
        assert!(run.accept_command(&rule).is_err());
    }
    #[test]
    fn symlink_and_oversize_input_are_rejected_before_spawn() {
        let f = Fixture::new("printf ok > out\n");
        let rule = f.rule(f.settings());
        fs::rename(f.root.join("input"), f.root.join("elsewhere")).unwrap();
        std::os::unix::fs::symlink("elsewhere", f.root.join("input")).unwrap();
        assert!(matches!(
            run_approved_command(&rule, &AtomicBool::new(false)),
            Err(CommandError::InputChanged)
        ));
        assert!(!f.root.join("out").exists());
        let f = Fixture::new("printf ok > out\n");
        let rule = f.rule(f.settings());
        fs::write(f.root.join("input"), vec![b'x'; 4097]).unwrap();
        assert!(matches!(
            run_approved_command(&rule, &AtomicBool::new(false)),
            Err(CommandError::InputChanged)
        ));
        assert!(!f.root.join("out").exists());
    }
    #[test]
    fn shell_native_mode_cannot_bypass_the_pinned_script_contract() {
        let f = Fixture::new("printf ok > out\n");
        let mut settings = f.settings();
        settings.mode = CommandMode::NativeExecutable;
        settings.arguments = vec!["approved-script".into()];
        assert!(matches!(
            run_approved_command(&f.rule(settings), &AtomicBool::new(false)),
            Err(CommandError::UnsupportedProgram)
        ));
        assert!(!f.root.join("out").exists());
    }
    #[test]
    fn script_cancel_timeout_and_capture_bounds_are_real_on_each_supported_platform() {
        use std::{
            sync::{atomic::Ordering, Arc},
            time::{Duration, Instant},
        };
        let f = Fixture::new("printf yes > started\nprintf ok > out\nwhile :; do :; done\n");
        let rule = f.rule(f.settings());
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = cancel.clone();
        let thread =
            std::thread::spawn(move || run_approved_command(&rule, &worker_cancel).unwrap());
        let deadline = Instant::now() + Duration::from_secs(2);
        while !f.root.join("started").exists() {
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(2));
        }
        cancel.store(true, Ordering::Relaxed);
        let run = thread.join().unwrap();
        assert_eq!(run.view().reason, RunReason::CancelRequested);
        assert!(run.view().cleanup.owned_leader_reaped);
        assert!(!run.view().passed);
        let mut settings = f.settings();
        settings.wall_time_ms = 30;
        let start = Instant::now();
        let run = run_approved_command(&f.rule(settings), &AtomicBool::new(false)).unwrap();
        assert_eq!(run.view().reason, RunReason::DeadlineExceeded);
        assert!(run.view().cleanup.owned_leader_reaped);
        assert!(start.elapsed() < Duration::from_secs(1));
        for redirect in ["", " >&2"] {
            let f = Fixture::new(&format!(
                "printf ok > out\nwhile :; do printf 0123456789{redirect}; done\n"
            ));
            let mut settings = f.settings();
            settings.stdout_limit = 0;
            settings.stderr_limit = 128;
            let run = run_approved_command(&f.rule(settings), &AtomicBool::new(false)).unwrap();
            assert_eq!(run.view().reason, RunReason::CaptureLimit);
            assert!(!run.view().passed);
            assert!(run.captured_stdout().is_empty());
            assert!(run.captured_stderr().len() <= 128);
        }
    }
    #[test]
    fn approved_script_background_pipe_group_is_retired_without_unbounded_reader_join() {
        use std::time::{Duration, Instant};
        let f = Fixture::new("( printf yes > child-ready; while :; do :; done ) &\nwhile [ ! -f child-ready ]; do :; done\nprintf ok > out\n");
        let start = Instant::now();
        let run = run_approved_command(&f.rule(f.settings()), &AtomicBool::new(false)).unwrap();
        assert!(f.root.join("child-ready").exists());
        assert!(run.view().cleanup.owned_leader_reaped);
        assert!(run.view().cleanup.owned_group_signal_sent);
        assert!(run.view().cleanup.all_capture_pipes_observed_eof);
        assert!(run.view().passed);
        assert!(start.elapsed() < Duration::from_secs(1));
        assert_eq!(
            run.view().cleanup.descendant_containment,
            "not_enforced_unknown"
        );
    }
    #[test]
    fn strict_isolation_cannot_be_silently_downgraded_for_scripts() {
        for index in 0..5 {
            let f = Fixture::new("printf ok > out\n");
            let mut settings = f.settings();
            match index {
                0 => settings.require.filesystem_isolation = true,
                1 => settings.require.network_isolation = true,
                2 => settings.require.process_tree_containment = true,
                3 => settings.require.execution_time_input_immutability = true,
                4 => settings.require.auxiliary_source_immutability = true,
                _ => unreachable!(),
            }
            assert!(matches!(
                run_approved_command(&f.rule(settings), &AtomicBool::new(false)),
                Err(CommandError::EnforcementUnavailable)
            ));
            assert!(!f.root.join("out").exists());
        }
    }
    #[test]
    fn expected_exit_zero_cannot_accept_missing_or_oversized_output() {
        for script in [
            "printf stdout-only\n",
            "while :; do printf 0123456789; done > out\n",
        ] {
            let f = Fixture::new(script);
            let mut settings = f.settings();
            settings.wall_time_ms = 30;
            let run = run_approved_command(&f.rule(settings), &AtomicBool::new(false)).unwrap();
            assert!(!run.view().passed);
            assert!(!run.view().file_integrity.unwrap().passed);
            if script.starts_with("printf") {
                assert_eq!(run.view().exit_code, Some(0));
                assert_eq!(run.view().reason, RunReason::ArtifactContractFailed);
            }
        }
    }
    #[test]
    fn artifact_fifo_hardlink_and_replaced_cwd_are_rejected_before_start() {
        use std::{
            ffi::CString,
            os::unix::{ffi::OsStrExt, fs::FileTypeExt},
            time::{Duration, Instant},
        };
        let f = Fixture::new("printf ok > out\n");
        let rule = f.rule(f.settings());
        fs::remove_file(f.root.join("input")).unwrap();
        let name = CString::new(f.root.join("input").as_os_str().as_bytes()).unwrap();
        assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) }, 0);
        assert!(fs::symlink_metadata(f.root.join("input"))
            .unwrap()
            .file_type()
            .is_fifo());
        let start = Instant::now();
        assert!(matches!(
            run_approved_command(&rule, &AtomicBool::new(false)),
            Err(CommandError::InputChanged)
        ));
        assert!(start.elapsed() < Duration::from_secs(1));
        assert!(!f.root.join("out").exists());
        let f = Fixture::new("printf ok > out\n");
        let rule = f.rule(f.settings());
        fs::hard_link(f.root.join("input"), f.root.join("alias")).unwrap();
        assert!(matches!(
            run_approved_command(&rule, &AtomicBool::new(false)),
            Err(CommandError::InputChanged)
        ));
        assert!(!f.root.join("out").exists());
        let f = Fixture::new("printf ok > out\n");
        let rule = f.rule(f.settings());
        let moved = f.root.with_extension("old");
        fs::rename(&f.root, &moved).unwrap();
        fs::create_dir(&f.root).unwrap();
        assert!(matches!(
            run_approved_command(&rule, &AtomicBool::new(false)),
            Err(CommandError::CwdChanged)
        ));
        assert!(!f.root.join("out").exists());
        fs::remove_dir(&f.root).unwrap();
        fs::rename(moved, &f.root).unwrap();
    }
    #[cfg(target_os = "macos")]
    #[test]
    fn macos_native_system_program_and_expected_exit_are_real() {
        let f = Fixture::new("unused");
        let executable = Path::new("/usr/bin/touch").canonicalize().unwrap();
        let build = |path: &Path, code, arguments| {
            let mut settings = f.settings();
            settings.mode = CommandMode::NativeExecutable;
            settings.arguments = arguments;
            settings.expected_exit_codes = vec![code];
            ApprovedCommandRule::from_verified_policy(
                "owner-native".into(),
                "policy-native".into(),
                ValidationSubject {
                    task_id: "task".into(),
                    attempt_id: "attempt".into(),
                    spec_revision: 1,
                    input_revision: "input-v1".into(),
                    version: VersionBinding::Data {
                        dataset_revision: "dataset-v1".into(),
                    },
                },
                f.artifact(
                    "native-program",
                    path.parent().unwrap(),
                    path.file_name().unwrap().to_str().unwrap(),
                    16 * 1024 * 1024,
                    Some(sha256(&fs::read(path).unwrap())),
                ),
                inspect(&f.root).unwrap(),
                vec![f.artifact("input", &f.root, "input", 4096, Some(sha256(b"input")))],
                vec![f.artifact("out", &f.root, "out", 4096, Some(sha256(b"")))],
                settings,
            )
            .unwrap()
        };
        let rule = build(&executable, 0, vec!["out".into()]);
        let run = run_approved_command(&rule, &AtomicBool::new(false)).unwrap();
        assert!(run.view().passed);
        assert!(f.root.join("out").exists());
        assert_eq!(run.view().protected_program.as_ref().unwrap().uid, 0);
        let false_program = Path::new("/usr/bin/false").canonicalize().unwrap();
        let rule = build(&false_program, 1, vec![]);
        let run = run_approved_command(&rule, &AtomicBool::new(false)).unwrap();
        assert!(run.view().passed);
        assert_eq!(run.view().exit_code, Some(1));
    }
    #[cfg(target_os = "macos")]
    #[test]
    fn macos_refuses_writable_program_chain_and_unavailable_sealing_before_spawn() {
        let f = Fixture::new("printf ok > out\n");
        let mut settings = f.settings();
        settings.require.sealed_executable_image = true;
        assert!(matches!(
            run_approved_command(&f.rule(settings), &AtomicBool::new(false)),
            Err(CommandError::EnforcementUnavailable)
        ));
        assert!(!f.root.join("out").exists());
        let mut f = Fixture::new("printf ok > out\n");
        let copy = f.root.join("writable-sh");
        fs::copy(&f.shell, &copy).unwrap();
        f.shell = copy;
        assert!(matches!(
            run_approved_command(&f.rule(f.settings()), &AtomicBool::new(false)),
            Err(CommandError::EnforcementUnavailable)
        ));
        assert!(!f.root.join("out").exists());
    }
}
