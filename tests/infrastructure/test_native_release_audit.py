"""Release-audit shell canaries for byte fixtures and synthetic app layouts.

Only the audit script and inspection tools run. Fixture bytes are never executed.
"""
import hashlib
import os
from pathlib import Path
import plistlib
import shlex
import shutil
import stat
import struct
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
AUDIT = ROOT / "scripts/native-audit-release.sh"
OLD_MARKERS = (
    "native_background_service_fixture_report",
    "native_background_fixture_alarm",
    "abg-registration-fixture-v1",
    "native_smoke_report",
    "native-smoke-create",
    "AGENTBATON_SMOKE_COMMIT",
    "owner_dialog_report",
    "native_factory_planning_os_dialog_fixture",
    "agentbaton_preview_evidence_gate_v1",
    ".agentbaton-preview-evidence-v1.json",
    ".agentbaton-preview-evidence-ready-v1.json",
    "agentbaton_copy_evidence_gate_v1",
    ".agentbaton-copy-evidence-v1.json",
    ".agentbaton-copy-evidence-ready-v1.json",
)
INTEGRATION_MARKER = "native_integration_os_dialog_fixture"
PASS = "PASS no native harness identifiers in default application executable"
BUNDLE_PASS = "PASS no inspected native harness identifiers in pinned ARM64 bundle executables"
ROLE_MARKERS = {
    "agentbaton-desktop": b"__TAURI_INTERNALS__",
    "baton": b"Usage: baton connections list/propose",
    "batond": b"batond API v1 ready at ",
}


class AuditShellTest(unittest.TestCase):
    required_tools = ("strings", "grep", "mktemp", "rm")

    def setUp(self):
        self.directory = tempfile.TemporaryDirectory(prefix="native-release-canary-")
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name).resolve(strict=True)
        self.assertEqual(stat.S_IMODE(self.root.stat().st_mode), 0o700)
        self.tools = self.root / "tools"
        self.inspection = self.root / "inspection"
        self.tools.mkdir(mode=0o700)
        self.inspection.mkdir(mode=0o700)
        self.bash = shutil.which("bash")
        self.assertIsNotNone(self.bash, "bash is required; missing tools are not a skip")
        # Real-tool cases use precisely discovered local tool directories. No
        # caller BASH_ENV/ENV/options or credentials are inherited by this shell.
        paths = []
        for tool in self.required_tools:
            program = shutil.which(tool)
            self.assertIsNotNone(program, f"{tool} is required; no environment skip")
            directory = str(Path(program).resolve(strict=True).parent)
            if directory not in paths:
                paths.append(directory)
        self.path = os.pathsep.join(paths)

    def run_audit(self, args, mocked=False, missing_tools=False):
        path = str(self.tools) if missing_tools else self.path
        if mocked:
            path = str(self.tools) + os.pathsep + path
        reply = subprocess.run(
            [self.bash, str(AUDIT), *map(str, args)],
            cwd=self.root,
            env={"PATH": path, "LC_ALL": "C", "TMPDIR": str(self.inspection)},
            capture_output=True,
            text=True,
            timeout=5,
            check=False,
        )
        self.assertEqual(list(self.inspection.iterdir()), [], "inspection trap must clean up")
        return reply

    def mock(self, name, body):
        program = self.tools / name
        program.write_text(f"#!{self.bash}\n{body}\n", encoding="utf-8")
        program.chmod(0o700)


class NativeReleaseAuditTests(AuditShellTest):
    def setUp(self):
        super().setUp()
        self.image = self.root / "non-executable-bytes.dat"

    def fixture(self, payload):
        with self.image.open("xb") as stream:
            stream.write(payload)
        self.image.chmod(0o600)
        return self.image

    def invoke(self, image=None, mocked=False, missing_tools=False):
        image = self.image if image is None else image
        before = None
        if image.is_file():
            before = (image.read_bytes(), image.stat())
        reply = self.run_audit([image], mocked=mocked, missing_tools=missing_tools)
        if before is not None:
            data, old = before
            current = image.stat()
            self.assertEqual(image.read_bytes(), data)
            self.assertEqual(
                (current.st_dev, current.st_ino, current.st_mode, current.st_nlink,
                 current.st_size, current.st_mtime_ns, current.st_ctime_ns),
                (old.st_dev, old.st_ino, old.st_mode, old.st_nlink,
                 old.st_size, old.st_mtime_ns, old.st_ctime_ns),
            )
            self.assertEqual(stat.S_IMODE(current.st_mode), 0o600)
            self.assertEqual(current.st_nlink, 1)
        return reply

    def assert_rejected_marker(self, marker):
        self.fixture(b"\x00generated non-executable audit fixture\x00" + marker.encode() + b"\x00")
        reply = self.invoke()
        self.assertEqual(reply.returncode, 1)
        self.assertNotIn(PASS, reply.stdout)
        self.assertIn("Test-only native harness content", reply.stderr)
        self.assertIn(marker, reply.stdout)
        self.image.unlink()

    def test_every_existing_marker_is_independently_rejected(self):
        for marker in OLD_MARKERS:
            with self.subTest(marker=marker):
                self.assert_rejected_marker(marker)

    def test_new_integration_os_test_marker_is_rejected(self):
        self.assert_rejected_marker(INTEGRATION_MARKER)

    def test_legitimate_integration_production_identifiers_are_not_test_markers(self):
        self.fixture(b"\x00integration_policy_approve\x00integration_context_get\x00"
                     b"integrations_start\x00authorized_local_integration\x00"
                     b"native_integration_review\x00")
        reply = self.invoke()
        self.assertEqual(reply.returncode, 0)
        self.assertEqual(reply.stdout.strip(), PASS)
        self.assertEqual(reply.stderr, "")

    def test_strings_failure_after_clean_output_never_reports_pass(self):
        self.fixture(b"legitimate production bytes\x00")
        self.mock("strings", "printf 'clean inspection prefix\\n'; exit 37")
        reply = self.invoke(mocked=True)
        self.assertEqual(reply.returncode, 37)
        self.assertNotIn(PASS, reply.stdout)

    def test_strings_failure_after_forbidden_output_never_reports_pass(self):
        self.fixture(b"legitimate production bytes\x00")
        self.mock("strings", f"printf '{INTEGRATION_MARKER}\\n'; exit 38")
        reply = self.invoke(mocked=True)
        self.assertEqual(reply.returncode, 38)
        self.assertNotIn(PASS, reply.stdout)

    def test_grep_error_never_becomes_no_match_success(self):
        self.fixture(b"legitimate production bytes\x00")
        self.mock("grep", "exit 23")
        reply = self.invoke(mocked=True)
        self.assertEqual(reply.returncode, 23)
        self.assertNotIn(PASS, reply.stdout)
        self.assertIn("Native application inspection failed.", reply.stderr)

    def test_missing_inspector_fails_before_success_or_inspection(self):
        self.fixture(b"legitimate production bytes\x00")
        reply = self.invoke(missing_tools=True)
        self.assertNotEqual(reply.returncode, 0)
        self.assertNotIn(PASS, reply.stdout)

    def test_missing_input_fails_without_success(self):
        reply = self.invoke(self.root / "does-not-exist")
        self.assertNotEqual(reply.returncode, 0)
        self.assertNotIn(PASS, reply.stdout)


class NativeBundleAuditTests(AuditShellTest):
    required_tools = (*AuditShellTest.required_tools, "file", "shasum", "python3")

    def setUp(self):
        super().setUp()
        self.app = self.root / "Test App.app"
        self.macos = self.app / "Contents" / "MacOS"
        self.macos.mkdir(parents=True)
        self.plist = self.app / "Contents" / "Info.plist"
        self.plist.write_bytes(plistlib.dumps({"CFBundleExecutable": "agentbaton-desktop"}))
        for role in ROLE_MARKERS:
            self.write_slot(role, self.image_for(role))
        self.manifest = self.root / "expected.sha256"
        self.write_pins()

    @staticmethod
    def image_for(role, cpu=0x0100000C, extra=b""):
        # A thin executable header and cstring section for the real inspectors.
        # The fixture has no entry point or instructions.
        payload = b"\x00" + ROLE_MARKERS[role] + b"\x00" + extra
        segment_format = "<II16sQQQQiiII"
        section_format = "<16s16sQQIIIIIIII"
        command_size = struct.calcsize(segment_format) + struct.calcsize(section_format)
        offset = 32 + command_size
        length = offset + len(payload)
        header = struct.pack("<8I", 0xFEEDFACF, cpu, 0, 2, 1, command_size, 0, 0)
        segment = struct.pack(segment_format, 0x19, command_size, b"__TEXT", 0, length,
                              0, length, 1, 1, 1, 0)
        section = struct.pack(section_format, b"__cstring", b"__TEXT", offset, len(payload),
                              offset, 0, 0, 0, 2, 0, 0, 0)
        return header + segment + section + payload

    def write_slot(self, role, data):
        path = self.macos / role
        path.write_bytes(data)
        path.chmod(0o700)

    def write_pins(self):
        self.manifest.write_text("".join(
            f"{hashlib.sha256((self.macos / role).read_bytes()).hexdigest()}  {role}\n"
            for role in ROLE_MARKERS
        ), encoding="ascii")

    def snapshot(self):
        files = {}
        for path in [self.app, *self.app.rglob("*"), self.manifest]:
            if not path.exists() and not path.is_symlink():
                continue
            info = path.lstat()
            content = None
            if stat.S_ISREG(info.st_mode):
                content = path.read_bytes()
            elif stat.S_ISLNK(info.st_mode):
                content = path.readlink()
            files[path] = (content, info.st_dev, info.st_ino, info.st_mode, info.st_nlink,
                           info.st_size, info.st_mtime_ns, info.st_ctime_ns)
        return files

    def invoke_bundle(self, mocked=False):
        before = self.snapshot()
        result = self.run_audit(["--bundle", self.app, "--sha256-manifest", self.manifest],
                                mocked=mocked)
        self.assertEqual(self.snapshot(), before, "audit must leave the input bundle and pins intact")
        return result

    def assert_rejected(self, result, diagnostic):
        self.assertNotEqual(result.returncode, 0)
        self.assertNotIn(BUNDLE_PASS, result.stdout)
        self.assertIn(diagnostic, result.stderr)

    def test_clean_pinned_arm64_slots_are_scanned_read_only(self):
        result = self.invoke_bundle()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout.strip(), BUNDLE_PASS)

    def test_each_missing_slot_fails(self):
        for role in ROLE_MARKERS:
            with self.subTest(role=role):
                (self.macos / role).unlink()
                self.assert_rejected(self.invoke_bundle(), "must contain exactly")
                self.write_slot(role, self.image_for(role))

    def test_unexpected_native_smoke_binary_fails_inventory(self):
        self.write_slot("agentbaton-native-owner-dialog-smoke", b"local canary")
        self.assert_rejected(self.invoke_bundle(), "must contain exactly")

    def test_wrong_launch_entry_is_refused(self):
        self.plist.write_bytes(plistlib.dumps({"CFBundleExecutable": "baton"}))
        self.assert_rejected(self.invoke_bundle(), "unexpected desktop executable")

    def test_symlink_slot_is_refused(self):
        source = self.root / "other-baton"
        (self.macos / "baton").rename(source)
        (self.macos / "baton").symlink_to(source)
        self.assert_rejected(self.invoke_bundle(), "Invalid packaged executable: baton")

    def test_foreign_build_of_the_same_role_is_refused(self):
        self.write_slot("baton", self.image_for("baton") + b"\x00different build bytes\x00")
        self.assert_rejected(self.invoke_bundle(), "SHA-256 mismatch for baton")

    def test_role_swap_is_refused_even_with_matching_byte_digests(self):
        self.write_slot("baton", self.image_for("batond"))
        self.write_slot("batond", self.image_for("baton"))
        self.write_pins()
        self.assert_rejected(self.invoke_bundle(), "Executable role mismatch for baton")

    def test_mixed_architecture_is_refused_even_with_matching_byte_digests(self):
        self.write_slot("baton", self.image_for("baton", cpu=0x01000007))
        self.write_pins()
        self.assert_rejected(self.invoke_bundle(), "Expected a thin ARM64 Mach-O executable for baton")

    def test_each_role_is_checked_for_source_gated_markers(self):
        leak = {
            "agentbaton-desktop": "agentbaton-native-background-service-smoke",
            "baton": ".agentbaton-preview-evidence-v1.json",
            "batond": ".agentbaton-copy-evidence-ready-v1.json",
        }
        for role, marker in leak.items():
            with self.subTest(role=role):
                self.write_slot(role, self.image_for(role, extra=marker.encode() + b"\x00"))
                self.write_pins()
                result = self.invoke_bundle()
                self.assert_rejected(result, f"Test-only native harness content is present in {role}")
                self.assertIn(marker, result.stdout)
                self.write_slot(role, self.image_for(role))

    def test_all_pins_must_be_present_and_unique(self):
        original = self.manifest.read_text()
        row = original.splitlines()[0] + "\n"
        self.manifest.write_text(original + row)
        self.assert_rejected(self.invoke_bundle(), "one SHA-256 row per packaged executable")
        self.manifest.write_text(row)
        self.assert_rejected(self.invoke_bundle(), "three distinct SHA-256 digests")
        digest = row.split()[0]
        self.manifest.write_text("".join(f"{digest}  {role}\n" for role in ROLE_MARKERS))
        self.assert_rejected(self.invoke_bundle(), "three distinct SHA-256 digests")
        self.manifest.unlink()
        self.assert_rejected(self.invoke_bundle(), "manifest is missing")

    def test_strings_failure_on_the_last_role_never_reports_pass(self):
        strings = shlex.quote(shutil.which("strings"))
        self.mock("strings", f"""case "$3" in
  */batond) printf 'batond API v1 ready at generated\n'; exit 37 ;;
esac
exec {strings} "$@" """)
        result = self.invoke_bundle(mocked=True)
        self.assertEqual(result.returncode, 37)
        self.assertNotIn(BUNDLE_PASS, result.stdout)

    def test_file_inspection_failure_never_reports_pass(self):
        self.mock("file", "printf 'Mach-O 64-bit executable arm64\n'; exit 42")
        result = self.invoke_bundle(mocked=True)
        self.assertEqual(result.returncode, 42)
        self.assertNotIn(BUNDLE_PASS, result.stdout)

    def test_role_inspection_error_never_reports_pass(self):
        grep = shlex.quote(shutil.which("grep"))
        self.mock("grep", f'if [[ "$1" == -F ]]; then exit 23; fi\nexec {grep} "$@"')
        result = self.invoke_bundle(mocked=True)
        self.assertEqual(result.returncode, 23)
        self.assert_rejected(result, "Native application inspection failed")


if __name__ == "__main__":
    unittest.main()
