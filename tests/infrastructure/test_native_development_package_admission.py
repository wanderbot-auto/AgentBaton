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

# Disk-image commands are simulated here. The tests run the real package entry
# and bundle audit, including their comparison against the compiled originals.
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
elif tool == "hdiutil":
    operation = args[0]
    with report.open("a") as handle:
        print(operation, *args[1:], file=handle)
    if operation == "create":
        image = Path(args[-1])
        assert image.parent.parent.parent == root / "artifacts"
        assert image.parent.parent.name.startswith(".development.")
        if case != "existing":
            assert not (root / "artifacts/development").exists()
        shutil.copytree(args[args.index("-srcfolder") + 1], root / "image-content", symlinks=True)
        image.write_bytes(b"fixture DMG")
        sys.exit(7 if case == "create-error" else 0)
    elif operation == "attach":
        assert {"-readonly", "-noautoopen", "-nobrowse"} <= set(args)
        volume = args[args.index("-mountpoint") + 1]
        disk.write_text(json.dumps({"image": args[-1], "volume": volume}))
        if case == "attach-error":
            sys.exit(8)
        if case == "interrupt":
            os.kill(os.getppid(), signal.SIGTERM)
            sys.exit(0)
        shutil.copytree(root / "image-content", volume, dirs_exist_ok=True, symlinks=True)
    elif operation == "info":
        assert args == ["info", "-plist"]
        images = [{"image-path": str(root / "unrelated.dmg"),
                   "system-entities": [{"dev-entry": "/dev/disk19"}]}]
        if disk.exists():
            active = json.loads(disk.read_text())
            images.append({"image-path": active["image"],
                           "system-entities": [{"dev-entry": "/dev/disk7"},
                                               {"dev-entry": "/dev/disk8s1",
                                                "mount-point": active["volume"]}]})
        sys.stdout.buffer.write(plistlib.dumps({"images": images}))
    elif operation == "detach":
        assert args == ["detach", "/dev/disk7"]
        if case == "detach-error":
            sys.exit(9)
        active = json.loads(disk.read_text())
        for entry in Path(active["volume"]).iterdir():
            if entry.is_dir() and not entry.is_symlink():
                shutil.rmtree(entry)
            else:
                entry.unlink()
        disk.unlink()
    else:
        raise AssertionError(args)
elif tool == "python3":
    if args[:1] == ["-"] and Path(args[1]).name == "volume":
        from types import SimpleNamespace

        os.path.ismount = lambda volume: disk.exists()
        os.statvfs = lambda volume: SimpleNamespace(
            f_flag=0 if case == "writable" else os.ST_RDONLY)
        sys.argv = args
        exec(compile(sys.stdin.read(), "<package volume check>", "exec"))
    else:
        os.execv(sys.executable, [sys.executable, *args])
elif tool == "file":
    print("Mach-O 64-bit executable arm64")
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
                     "rustc", "node", "xcodebuild", "codesign"):
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
        return [line.split()[0] for line in (self.root / "commands").read_text().splitlines()]

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
        self.assertIn("PASS no inspected native harness identifiers", result.stdout)
        self.assertEqual(self.operations(), ["create", "attach", "info", "detach"])
        self.assertEqual({entry.name for entry in self.output.iterdir()},
                         {DMG, "SHA256SUMS", "build-record.txt", "DEVELOPMENT_ACCEPTANCE.md"})
        self.assertEqual((self.output / "SHA256SUMS").read_text(),
                         f"{hashlib.sha256((self.output / DMG).read_bytes()).hexdigest()}  ./{DMG}\n")
        record = (self.output / "build-record.txt").read_text().splitlines()
        self.assertEqual(record[:3], [f"commit={COMMIT}", "architecture=arm64",
                                      "channel=development; no Developer ID signing or notarization configured"])
        self.assertEqual((self.output / "DEVELOPMENT_ACCEPTANCE.md").read_bytes(),
                         (ROOT / "docs/execution/DEVELOPMENT_ACCEPTANCE.md").read_bytes())
        self.assertFalse((self.root / "attached").exists())
        self.assertEqual(list(self.artifacts.glob(".development.*")), [])

    def test_rejects_a_different_binary_for_the_same_role(self):
        with (self.macos / "batond").open("ab") as stream:
            stream.write(b"Different compiled artifact\n")
        self.assert_clean_failure(self.package(), "SHA-256 mismatch for batond",
                                  ["create", "attach", "info", "detach"])

    def test_rejects_a_missing_executable_in_the_image(self):
        (self.macos / "baton").unlink()
        self.assert_clean_failure(self.package(), "Contents/MacOS must contain exactly",
                                  ["create", "attach", "info", "detach"])

    def test_rejects_a_missing_compiled_original_before_creating_image(self):
        (self.desktop / "agentbaton-desktop").unlink()
        result = self.package()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Missing or invalid compiled development executable for agentbaton-desktop",
                      result.stdout)
        self.assertFalse(self.output.exists())
        self.assertFalse((self.root / "commands").exists())
        self.assertEqual(list(self.artifacts.glob(".development.*")), [])

    def test_create_failure_cleans_up_a_partial_image(self):
        result = self.package("create-error")
        self.assertEqual(result.returncode, 7, result.stdout)
        self.assert_clean_failure(result, "", ["create"])

    def test_attach_failure_detaches_the_owned_device_without_a_volume(self):
        result = self.package("attach-error")
        self.assertEqual(result.returncode, 8, result.stdout)
        self.assert_clean_failure(result, "", ["create", "attach", "info", "detach"])

    def test_term_after_attach_detaches_the_owned_device(self):
        result = self.package("interrupt")
        self.assertEqual(result.returncode, 143, result.stdout)
        self.assert_clean_failure(result, "", ["create", "attach", "info", "detach"])

    def test_rejects_a_writable_attachment(self):
        self.assert_clean_failure(self.package("writable"), "must be mounted read-only",
                                  ["create", "attach", "info", "detach"])

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
        self.assertEqual(self.operations(), ["create", "attach", "info", "detach", "info", "detach"])
        self.assertEqual(len(list(self.artifacts.glob(".development.*"))), 1)


if __name__ == "__main__":
    unittest.main()
