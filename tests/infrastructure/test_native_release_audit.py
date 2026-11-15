"""Real release-audit shell canaries, not Mach-O or a product release audit.

Inputs are private non-executable byte fixtures. Only bash/the existing audit
script/inspection tools run; fixture contents are never executed.
"""
import hashlib
import os
from pathlib import Path
import shutil
import stat
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
AUDIT = ROOT / "scripts/native-audit-release.sh"
OLD_MARKERS = (
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


class NativeReleaseAuditTests(unittest.TestCase):
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
        for tool in ("strings", "grep", "mktemp", "rm"):
            program = shutil.which(tool)
            self.assertIsNotNone(program, f"{tool} is required; no environment skip")
            directory = str(Path(program).resolve(strict=True).parent)
            if directory not in paths:
                paths.append(directory)
        self.path = os.pathsep.join(paths)
        self.image = self.root / "non-executable-bytes.dat"

    def fixture(self, payload):
        with self.image.open("xb") as stream:
            stream.write(payload)
        self.image.chmod(0o600)
        return self.image

    def mock(self, name, body):
        program = self.tools / name
        # Only these fixed tools are executable. No fixture bytes become code.
        program.write_text(f"#!{self.bash}\n{body}\n", encoding="utf-8")
        program.chmod(0o700)

    def invoke(self, image=None, mocked=False, missing_tools=False):
        image = self.image if image is None else image
        before = None
        if image.is_file():
            before = (image.read_bytes(), image.stat())
        path = str(self.tools) if missing_tools else self.path
        if mocked:
            path = str(self.tools) + os.pathsep + path
        reply = subprocess.run(
            [self.bash, str(AUDIT), str(image)],
            cwd=self.root,
            env={"PATH": path, "LC_ALL": "C", "TMPDIR": str(self.inspection)},
            capture_output=True,
            text=True,
            timeout=5,
            check=False,
        )
        self.assertEqual(list(self.inspection.iterdir()), [], "inspection trap must clean up")
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


if __name__ == "__main__":
    unittest.main()
