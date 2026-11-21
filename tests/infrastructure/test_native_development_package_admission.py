import hashlib
import os
from pathlib import Path
import plistlib
import shutil
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = "scripts/package-development-macos.sh"
COMMIT = "a" * 40
DMG = f"AgentBaton-arm64-{COMMIT}-development.dmg"
ROLES = {
    "agentbaton-desktop": b"__TAURI_INTERNALS__\n",
    "baton": b"Usage: baton connections list/propose\n",
    "batond": b"batond API v1 ready at \n",
}

SEAL = "Contents/_CodeSignature/CodeResources"
SEAL_BYTES = b"fixture resource seal"
SIGNATURE_SUFFIX = b"\nfixture app signature\n"
SIGNING_STEPS = ["verify-baton", "verify-batond", "sign-app", "verify-stage"]
MOUNT_STEPS = ["create", "attach"]
CLEANUP_STEPS = ["info", "detach"]
TRANSIENT_ERROR = "hdiutil: attach failed - Resource temporarily unavailable"

# Simulate signing and disk images. The package entry and both byte/role audits
# run as shipped against separately pinned compiled and signed inputs.
COMMAND = r"""
import json
import os
from pathlib import Path
import plistlib
import shutil
import signal
import sys

root = Path(os.environ["PACKAGE_FIXTURE"])
case = os.environ["PACKAGE_CASE"]
tool, *args = sys.argv
tool = Path(tool).name
disk = root / "attached"
report = root / "commands"
seal = "Contents/_CodeSignature/CodeResources"

def record(operation):
    with report.open("a") as handle:
        print(operation, file=handle)

def append(path, data):
    with path.open("ab") as handle:
        handle.write(data)

if tool == "uname":
    print({"-s": "Darwin", "-m": "arm64"}[args[0]])
elif tool == "git":
    print("a" * 40)
elif tool == "cargo":
    assert args[0] == "metadata"
    manifest = args[args.index("--manifest-path") + 1]
    target = root / Path(manifest).parent / "target"
    print(json.dumps({"target_directory": str(target)}))
elif tool == "ditto":
    shutil.copytree(*args)
elif tool == "codesign":
    target = Path(args[-1])
    path = target.relative_to(root / "artifacts")
    assert path.parts[0].startswith(".development."), target
    location = path.parts[1]
    if args[0] == "--force":
        assert args[:-1] == ["--force", "--sign", "-", "--timestamp=none"], args
        assert location == "stage" and target.name == "AgentBaton.app", target
        record("sign-app")
        if case == "sign-error":
            sys.exit("Simulated ad hoc signing failure")
        append(target / "Contents/MacOS/agentbaton-desktop", b"\nfixture app signature\n")
        (target / seal).parent.mkdir()
        (target / seal).write_bytes(b"fixture resource seal")
        if case == "sign-changes-helper":
            append(target / "Contents/MacOS/batond", b"Unexpected sidecar rewrite\n")
    elif args[0] == "--verify" and target.name in ("baton", "batond"):
        assert args[:-1] == ["--verify", "--strict"] and location == "stage", args
        record("verify-" + target.name)
        if case == "invalid-helper" and target.name == "baton":
            sys.exit("Invalid helper signature")
    elif args[0] == "--verify":
        assert args[:-1] == ["--verify", "--deep", "--strict"], args
        assert target.name == "AgentBaton.app" and (target / seal).is_file(), target
        record("verify-" + location)
        if location == "stage" and case == "invalid-stage":
            sys.exit("Invalid staging signature")
        if location == "volume" and case == "invalid-volume":
            sys.exit("Invalid mounted signature")
    elif args[0] == "--display":
        assert args[:-1] == ["--display", "--verbose=4"] and location == "volume", args
        record("display-volume")
        print(f"Executable={target}/Contents/MacOS/agentbaton-desktop", file=sys.stderr)
        print("Signature=adhoc" if case != "unexpected-signer" else "Signature=other", file=sys.stderr)
        print("TeamIdentifier=not set", file=sys.stderr)
        print("Sealed Resources version=2 rules=13 files=3", file=sys.stderr)
    else:
        raise AssertionError(args)
elif tool == "hdiutil":
    operation = args[0]
    record(operation)
    if operation == "create":
        image = Path(args[-1])
        assert image.parent.parent.parent == root / "artifacts"
        assert image.parent.parent.name.startswith(".development.")
        if case != "existing":
            assert not (root / "artifacts/development").exists()
        shutil.copytree(args[args.index("-srcfolder") + 1], root / "image-content", symlinks=True)
        copied = root / "image-content/AgentBaton.app"
        if case == "changed-volume-binary":
            append(copied / "Contents/MacOS/agentbaton-desktop", b"Replaced packaged executable\n")
        if case == "missing-volume-binary":
            (copied / "Contents/MacOS/baton").unlink()
        if case == "changed-volume-seal":
            append(copied / seal, b"Replaced resource seal")
        if case == "missing-volume-seal":
            (copied / seal).unlink()
        image.write_bytes(b"fixture DMG")
        sys.exit(7 if case == "create-error" else 0)
    elif operation == "attach":
        assert {"-readonly", "-noautoopen", "-nobrowse"} <= set(args)
        volume = args[args.index("-mountpoint") + 1]
        attempt = report.read_text().splitlines().count("attach")
        assert args == ["attach", "-readonly", "-noautoopen", "-nobrowse", "-mountpoint", volume, args[-1]]
        attachment = root / "attach-args.json"
        if attempt == 1:
            attachment.write_text(json.dumps(args))
        else:
            assert args == json.loads(attachment.read_text())
        assert not disk.exists()
        assert Path(volume).is_dir() and not list(Path(volume).iterdir())
        if case == "transient-no-device" and attempt == 1:
            sys.exit("hdiutil: attach failed - Resource temporarily unavailable")
        disk.write_text(json.dumps({"image": args[-1], "volume": volume}))
        if case == "transient-exhausted" or (case.startswith("transient-") and attempt == 1):
            assert os.environ["LC_ALL"] == "C"
            sys.exit("hdiutil: attach failed - Resource temporarily unavailable")
        if case == "unknown-attach-error":
            sys.exit("hdiutil: attach failed - Permission denied")
        if case == "transient-then-unknown":
            print("hdiutil: attach failed - Permission denied", file=sys.stderr)
            sys.exit(4)
        if case == "different-status":
            print("hdiutil: attach failed - Resource temporarily unavailable", file=sys.stderr)
            sys.exit(8)
        if case == "mixed-attach-error":
            sys.exit("hdiutil: attach failed - Resource temporarily unavailable\nUnexpected secondary error")
        if case == "nul-attach-error":
            sys.stderr.buffer.write(b"hdiutil: attach failed - Resource temporarily unavailable\x00\n")
            sys.exit(1)
        if case == "attach-error":
            sys.exit(8)
        if case == "interrupt":
            os.kill(os.getppid(), signal.SIGTERM)
            sys.exit(0)
        shutil.copytree(root / "image-content", volume, dirs_exist_ok=True, symlinks=True)
        if case == "attach-changes-image":
            append(Path(args[-1]), b"Unexpected image change")
    elif operation == "info":
        assert args == ["info", "-plist"]
        if case == "transient-info-error" or (case == "transient-verify-info-error" and not disk.exists()):
            sys.exit("Simulated attachment inventory failure")
        images = [{"image-path": str(root / "unrelated.dmg"),
                   "system-entities": [{"dev-entry": "/dev/disk19"}]}]
        if disk.exists():
            active = json.loads(disk.read_text())
            images.append({"image-path": active["image"],
                           "system-entities": [{"dev-entry": "/dev/disk7"},
                                               {"dev-entry": "/dev/disk8s1",
                                                "mount-point": active["volume"]}]})
            if case == "transient-ambiguous":
                images.append(images[-1])
            if case == "transient-invalid-inventory":
                images[-1].pop("image-path")
            if case == "transient-foreign-disk":
                images[0]["system-entities"][0]["dev-entry"] = "/dev/disk7s1"
            if case == "transient-foreign-mount":
                images[0]["system-entities"][0]["mount-point"] = active["volume"]
        sys.stdout.buffer.write(plistlib.dumps({"images": images}))
    elif operation == "detach":
        assert args == ["detach", "/dev/disk7"]
        if case in ("detach-error", "transient-detach-error"):
            sys.exit(9)
        if case == "transient-detach-remains":
            sys.exit(0)
        active = json.loads(disk.read_text())
        for entry in Path(active["volume"]).iterdir():
            if entry.is_dir() and not entry.is_symlink():
                shutil.rmtree(entry)
            else:
                entry.unlink()
        disk.unlink()
        print('"disk7" ejected.')
        if case == "transient-image-drift":
            append(Path(active["image"]), b"Unexpected image change")
    else:
        raise AssertionError(args)
elif tool == "python3":
    if args[:1] == ["-"] and Path(args[1]).name == "volume":
        from types import SimpleNamespace

        os.path.ismount = lambda volume: disk.exists()
        os.statvfs = lambda volume: SimpleNamespace(
            f_flag=0 if case in ("writable", "transient-then-writable") else os.ST_RDONLY)
        sys.argv = args
        exec(compile(sys.stdin.read(), "<package volume check>", "exec"))
    else:
        os.execv(sys.executable, [sys.executable, *args])
elif tool == "file":
    print("Mach-O 64-bit executable arm64")
elif tool == "sleep":
    assert args in (["1"], ["2"])
    record("sleep-" + args[0])
    if case == "transient-reappeared":
        attach = json.loads((root / "attach-args.json").read_text())
        disk.write_text(json.dumps({"image": attach[-1], "volume": attach[-2]}))
else:
    print(tool, *args)
"""


class NativeDevelopmentPackageAdmissionTests(unittest.TestCase):
    def setUp(self):
        directory = tempfile.TemporaryDirectory(prefix="ab-package-admission-")
        self.addCleanup(directory.cleanup)
        self.root = Path(directory.name).resolve()
        for path in (SCRIPT, "scripts/native-audit-release-bundle.sh", "scripts/native-audit-release.sh",
                     "docs/execution/DEVELOPMENT_ACCEPTANCE.md"):
            destination = self.root / path
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(ROOT / path, destination)
        self.desktop = self.root / "apps/desktop/src-tauri/target/release"
        contents = self.desktop / "bundle/macos/AgentBaton.app/Contents"
        self.macos = contents / "MacOS"
        self.macos.mkdir(parents=True)
        (contents / "Info.plist").write_bytes(plistlib.dumps({"CFBundleExecutable": "agentbaton-desktop"}))
        workspace = self.root / "target/release"
        workspace.mkdir(parents=True)
        for role, data in ROLES.items():
            source = (self.desktop if role == "agentbaton-desktop" else workspace) / role
            source.write_bytes(data)
            source.chmod(0o700)
            shutil.copy2(source, self.macos / role)

        commands = self.root / "commands-bin"
        commands.mkdir()
        command = commands / "command"
        command.write_text(f"#!{sys.executable}\n" + COMMAND)
        command.chmod(0o700)
        for name in ("git", "uname", "cargo", "hdiutil", "ditto", "python3", "file", "sw_vers",
                     "rustc", "node", "xcodebuild", "codesign", "sleep"):
            (commands / name).symlink_to(command.name)
        self.env = dict(os.environ, PATH=f"{commands}{os.pathsep}{os.environ['PATH']}",
                        PACKAGE_FIXTURE=str(self.root))
        self.artifacts = self.root / "artifacts"
        self.output = self.artifacts / "development"

    def package(self, case="normal"):
        return subprocess.run(
            ["bash", SCRIPT], cwd=self.root, env=dict(self.env, PACKAGE_CASE=case),
            stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, timeout=30,
        )

    def operations(self):
        log = self.root / "commands"
        return log.read_text().splitlines() if log.exists() else []

    def assert_clean_failure(self, result, error, operations):
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn(error, result.stdout)
        self.assertEqual(self.operations(), operations)
        self.assertFalse(self.output.exists())
        self.assertFalse((self.root / "attached").exists())
        self.assertEqual(list(self.artifacts.glob(".development.*")), [])

    def test_publishes_the_existing_four_file_contract_after_audit_and_detach(self):
        result = self.package()
        self.assertEqual(result.returncode, 0, result.stdout)
        self.assertEqual(result.stdout.count("PASS no inspected native harness identifiers"), 2)
        self.assertEqual(self.operations(), SIGNING_STEPS + MOUNT_STEPS
                         + ["verify-volume", "display-volume"] + CLEANUP_STEPS)
        self.assertEqual({entry.name for entry in self.output.iterdir()},
                         {DMG, "SHA256SUMS", "build-record.txt", "DEVELOPMENT_ACCEPTANCE.md"})
        self.assertEqual((self.output / "SHA256SUMS").read_text(),
                         f"{hashlib.sha256((self.output / DMG).read_bytes()).hexdigest()}  ./{DMG}\n")
        record = (self.output / "build-record.txt").read_text().splitlines()
        self.assertEqual(record[:3], [f"commit={COMMIT}", "architecture=arm64",
                                      "channel=development; no Developer ID signing or notarization configured"])
        evidence = dict(line.split("=", 1) for line in record if "=" in line)
        self.assertEqual(evidence["signing_mode"], "ad_hoc_app")
        self.assertEqual(evidence["Signature"], "adhoc")
        self.assertEqual(evidence["dmg_sha256"], hashlib.sha256((self.output / DMG).read_bytes()).hexdigest())
        self.assertEqual(evidence["code_resources_sha256"], hashlib.sha256(SEAL_BYTES).hexdigest())
        self.assertNotIn("Executable", evidence)
        for role, compiled in ROLES.items():
            with self.subTest(role=role):
                shipped = compiled + SIGNATURE_SUFFIX if role == "agentbaton-desktop" else compiled
                self.assertEqual(evidence[f"compiled_sha256.{role}"], hashlib.sha256(compiled).hexdigest())
                self.assertEqual(evidence[f"shipped_sha256.{role}"], hashlib.sha256(shipped).hexdigest())
                self.assertEqual((self.macos / role).read_bytes(), compiled)
                self.assertEqual((self.root / "image-content/AgentBaton.app/Contents/MacOS" / role).read_bytes(),
                                 shipped)
        self.assertEqual((self.output / "DEVELOPMENT_ACCEPTANCE.md").read_bytes(),
                         (ROOT / "docs/execution/DEVELOPMENT_ACCEPTANCE.md").read_bytes())
        self.assertFalse((self.root / "attached").exists())
        self.assertEqual(list(self.artifacts.glob(".development.*")), [])

    def test_rejects_a_different_binary_for_the_same_role(self):
        with (self.macos / "batond").open("ab") as stream:
            stream.write(b"Different compiled artifact\n")
        self.assert_clean_failure(self.package(), "SHA-256 mismatch for batond", [])

    def test_rejects_a_missing_staged_executable_before_signing(self):
        (self.macos / "baton").unlink()
        self.assert_clean_failure(self.package(), "Contents/MacOS must contain exactly", [])

    def test_rejects_a_missing_compiled_original_before_creating_image(self):
        (self.desktop / "agentbaton-desktop").unlink()
        result = self.package()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Missing or invalid compiled development executable for agentbaton-desktop",
                      result.stdout)
        self.assertFalse(self.output.exists())
        self.assertFalse((self.root / "commands").exists())
        self.assertEqual(list(self.artifacts.glob(".development.*")), [])

    def test_invalid_helper_blocks_outer_signing(self):
        self.assert_clean_failure(self.package("invalid-helper"), "Invalid helper signature", ["verify-baton"])

    def test_outer_signing_failure_blocks_image_creation(self):
        self.assert_clean_failure(self.package("sign-error"), "Simulated ad hoc signing failure", SIGNING_STEPS[:-1])

    def test_invalid_staged_signature_blocks_image_creation(self):
        self.assert_clean_failure(self.package("invalid-stage"), "Invalid staging signature", SIGNING_STEPS)

    def test_outer_signing_must_leave_helper_bytes_unchanged(self):
        self.assert_clean_failure(self.package("sign-changes-helper"), "unexpectedly changed batond", SIGNING_STEPS)

    def test_mounted_layout_and_pins_are_checked_against_signed_staging(self):
        outcomes = [
            ("missing-volume-binary", "Contents/MacOS must contain exactly"),
            ("changed-volume-binary", "SHA-256 mismatch for agentbaton-desktop"),
            ("missing-volume-seal", "Missing or invalid development app resource seal"),
            ("changed-volume-seal", "resource seal SHA-256 mismatch"),
        ]
        for case, error in outcomes:
            with self.subTest(case=case):
                self.assert_clean_failure(self.package(case), error, SIGNING_STEPS + MOUNT_STEPS + CLEANUP_STEPS)
                (self.root / "commands").unlink()
                shutil.rmtree(self.root / "image-content")

    def test_mounted_strict_failure_prevents_publication(self):
        self.assert_clean_failure(self.package("invalid-volume"), "Invalid mounted signature",
                                  SIGNING_STEPS + MOUNT_STEPS + ["verify-volume"] + CLEANUP_STEPS)

    def test_mounted_signer_must_be_ad_hoc(self):
        self.assert_clean_failure(self.package("unexpected-signer"), "Expected an ad hoc development app signature",
                                  SIGNING_STEPS + MOUNT_STEPS
                                  + ["verify-volume", "display-volume"] + CLEANUP_STEPS)

    def test_create_failure_cleans_up_a_partial_image(self):
        result = self.package("create-error")
        self.assertEqual(result.returncode, 7, result.stdout)
        self.assert_clean_failure(result, "", SIGNING_STEPS + ["create"])

    def test_attach_failure_detaches_the_owned_device_without_a_volume(self):
        result = self.package("attach-error")
        self.assertEqual(result.returncode, 8, result.stdout)
        self.assert_clean_failure(result, "", SIGNING_STEPS + MOUNT_STEPS + CLEANUP_STEPS)

    def test_exact_transient_attach_reconciles_owned_device_and_retries_read_only(self):
        result = self.package("transient-once")
        self.assertEqual(result.returncode, 0, result.stdout)
        self.assertIn(TRANSIENT_ERROR, result.stdout)
        self.assertEqual(result.stdout.count('"disk7" ejected.'), 2)
        self.assertIn("Confirmed failed mount cleanup", result.stdout)
        self.assertEqual(result.stdout.count("PASS no inspected native harness identifiers"), 2)
        self.assertEqual(self.operations(), SIGNING_STEPS + MOUNT_STEPS + CLEANUP_STEPS
                         + ["sleep-1", "info", "attach", "verify-volume", "display-volume"] + CLEANUP_STEPS)
        self.assertEqual({entry.name for entry in self.output.iterdir()},
                         {DMG, "SHA256SUMS", "build-record.txt", "DEVELOPMENT_ACCEPTANCE.md"})
        self.assertFalse((self.root / "attached").exists())
        self.assertEqual(list(self.artifacts.glob(".development.*")), [])

    def test_exact_transient_without_a_device_verifies_absence_without_detaching_foreign_image(self):
        result = self.package("transient-no-device")
        self.assertEqual(result.returncode, 0, result.stdout)
        self.assertIn(TRANSIENT_ERROR, result.stdout)
        self.assertEqual(self.operations(), SIGNING_STEPS + MOUNT_STEPS
                         + ["info", "sleep-1", "info", "attach", "verify-volume", "display-volume"] + CLEANUP_STEPS)
        self.assertEqual(result.stdout.count('"disk7" ejected.'), 1)
        self.assertFalse((self.root / "attached").exists())

    def test_persistent_transient_fails_after_three_attachments_and_confirmed_cleanup(self):
        result = self.package("transient-exhausted")
        self.assertEqual(result.returncode, 1, result.stdout)
        self.assertEqual(result.stdout.count(TRANSIENT_ERROR), 3)
        self.assertEqual(result.stdout.count('"disk7" ejected.'), 3)
        recovery = ["attach", "info", "detach"]
        self.assert_clean_failure(result, "mount retry limit reached",
                                  SIGNING_STEPS + ["create"] + recovery + ["sleep-1", "info"] + recovery
                                  + ["sleep-2", "info"] + recovery + ["info"])

    def test_uncertain_cleanup_never_retries_or_removes_owned_image_evidence(self):
        cases = (
            ("transient-info-error", ["info"], "inventory failure", True),
            ("transient-invalid-inventory", ["info"], "Cannot read development image attachments", True),
            ("transient-ambiguous", ["info"], "Ambiguous development image attachment", True),
            ("transient-foreign-disk", ["info"], "also listed for another image", True),
            ("transient-foreign-mount", ["info"], "Unexpected ownership", True),
            ("transient-detach-error", CLEANUP_STEPS, "cleanup failed", True),
            ("transient-detach-remains", CLEANUP_STEPS + ["sleep-1", "info"], "still attached", True),
            ("transient-reappeared", CLEANUP_STEPS + ["sleep-1", "info"], "still attached", True),
            ("transient-verify-info-error", CLEANUP_STEPS + ["sleep-1", "info"], "inventory failure", False),
        )
        for case, cleanup, error, attached in cases:
            with self.subTest(case=case):
                result = self.package(case)
                self.assertEqual(result.returncode, 1, result.stdout)
                self.assertIn(TRANSIENT_ERROR, result.stdout)
                self.assertIn(error, result.stdout)
                self.assertIn("Private files remain at", result.stdout)
                self.assertEqual(self.operations(), SIGNING_STEPS + MOUNT_STEPS + cleanup)
                self.assertFalse(self.output.exists())
                self.assertEqual((self.root / "attached").exists(), attached)
                private, = self.artifacts.glob(".development.*")
                self.assertTrue((private / "output" / DMG).is_file())
                self.assertEqual((private / "attach-1.stderr").read_text().strip(), TRANSIENT_ERROR)
                (self.root / "attached").unlink(missing_ok=True)
                (self.root / "commands").unlink()
                shutil.rmtree(self.root / "image-content")
                shutil.rmtree(private)

    def test_other_errors_or_statuses_keep_stderr_and_do_not_retry(self):
        cases = (
            ("unknown-attach-error", 1, "Permission denied"),
            ("different-status", 8, TRANSIENT_ERROR),
            ("mixed-attach-error", 1, "Unexpected secondary error"),
            ("nul-attach-error", 1, TRANSIENT_ERROR),
        )
        for case, code, error in cases:
            with self.subTest(case=case):
                result = self.package(case)
                self.assertEqual(result.returncode, code, result.stdout)
                self.assert_clean_failure(result, error, SIGNING_STEPS + MOUNT_STEPS + CLEANUP_STEPS)
                (self.root / "commands").unlink()
                shutil.rmtree(self.root / "image-content")
        result = self.package("transient-then-unknown")
        self.assertEqual(result.returncode, 4, result.stdout)
        self.assertIn(TRANSIENT_ERROR, result.stdout)
        self.assert_clean_failure(result, "Permission denied", SIGNING_STEPS + MOUNT_STEPS + CLEANUP_STEPS
                                  + ["sleep-1", "info", "attach"] + CLEANUP_STEPS)

    def test_recovery_and_final_publication_reject_changed_image_bytes(self):
        cases = (
            ("transient-image-drift", "bytes changed before mount retry", CLEANUP_STEPS + ["sleep-1", "info"]),
            ("attach-changes-image", "bytes changed during mount verification",
             ["verify-volume", "display-volume"] + CLEANUP_STEPS),
        )
        for case, error, tail in cases:
            with self.subTest(case=case):
                self.assert_clean_failure(self.package(case), error, SIGNING_STEPS + MOUNT_STEPS + tail)
                (self.root / "commands").unlink()
                shutil.rmtree(self.root / "image-content")

    def test_recovered_attach_must_still_pass_read_only_and_signing_admission(self):
        self.assert_clean_failure(self.package("transient-then-writable"), "must be mounted read-only",
                                  SIGNING_STEPS + MOUNT_STEPS + CLEANUP_STEPS
                                  + ["sleep-1", "info", "attach"] + CLEANUP_STEPS)

    def test_term_after_attach_detaches_the_owned_device(self):
        result = self.package("interrupt")
        self.assertEqual(result.returncode, 143, result.stdout)
        self.assert_clean_failure(result, "", SIGNING_STEPS + MOUNT_STEPS + CLEANUP_STEPS)

    def test_rejects_a_writable_attachment(self):
        self.assert_clean_failure(self.package("writable"), "must be mounted read-only",
                                  SIGNING_STEPS + MOUNT_STEPS + CLEANUP_STEPS)

    def test_existing_output_is_preserved_if_publication_cannot_replace_it(self):
        self.output.mkdir(parents=True)
        previous = self.output / DMG
        previous.write_bytes(b"previous package")
        result = self.package("existing")
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn("Cannot publish development package", result.stdout)
        self.assertEqual(list(self.output.iterdir()), [previous])
        self.assertEqual(previous.read_bytes(), b"previous package")
        self.assertFalse((self.root / "attached").exists())
        self.assertEqual(list(self.artifacts.glob(".development.*")), [])

    def test_detach_failure_blocks_publication_and_retains_private_cleanup_path(self):
        result = self.package("detach-error")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Development image cleanup failed. Private files remain at", result.stdout)
        self.assertFalse(self.output.exists())
        self.assertTrue((self.root / "attached").exists())
        self.assertEqual(self.operations(), SIGNING_STEPS + MOUNT_STEPS
                         + ["verify-volume", "display-volume"] + CLEANUP_STEPS * 2)
        self.assertEqual(len(list(self.artifacts.glob(".development.*"))), 1)


if __name__ == "__main__":
    unittest.main()
