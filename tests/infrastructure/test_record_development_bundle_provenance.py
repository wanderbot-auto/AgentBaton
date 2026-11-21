import importlib.util
import json
import pathlib
import plistlib
import struct
import subprocess
import sys
import tempfile
import unittest
import warnings
import zipfile
from contextlib import nullcontext
from types import SimpleNamespace
from unittest.mock import patch

ROOT = pathlib.Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts/record-development-bundle-provenance.py"
SPEC = importlib.util.spec_from_file_location("bundle_provenance", SCRIPT)
PROVENANCE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PROVENANCE)

COMMIT = "a" * 40
TREE = "b" * 40
DMG = f"AgentBaton-arm64-{COMMIT}-development.dmg"
APP = "AgentBaton.app/Contents"
DESKTOP = APP + "/MacOS/agentbaton-desktop"
CLI = APP + "/MacOS/baton"
DAEMON = APP + "/MacOS/batond"
ICON = APP + "/Resources/icon.icns"
PLIST = APP + "/Info.plist"
CHECKLIST = b"development checklist fixture\n"
CHANNEL = "development; no Developer ID signing or notarization configured"


def macho(cpu=0x0100000C, command_size=8, tail=b""):
    return struct.pack("<10I", 0xFEEDFACF, cpu, 0, 2, 1, 8, 0, 0, 0x2, command_size) + tail


class ProvenanceTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = pathlib.Path(temporary.name)
        self.archive = self.root / "development.zip"
        self.volume = self.root / "volume"
        self.volume.mkdir()
        (self.volume / "Applications").symlink_to("/Applications")
        self.inputs = {
            "commit": COMMIT, "tree": TREE, "version": "0.1.0", "identifier": "dev.agentbaton",
            "config_sha256": "f" * 64,
            "checklist_sha256": PROVENANCE.digest(CHECKLIST),
            "executables": {DESKTOP: "desktop", CLI: "cli", DAEMON: "daemon"},
            "repo_resources": {ICON: {"path": "icons/icon.icns", "sha256": PROVENANCE.digest(b"icon")}},
        }
        self.put(DESKTOP, macho(tail=b"const fixture = 5;\n"))
        self.put(CLI, macho())
        self.put(DAEMON, macho(tail=b"daemon"))
        self.put(ICON, b"icon")
        self.put(PLIST, plistlib.dumps({
            "CFBundleIdentifier": "dev.agentbaton", "CFBundleShortVersionString": "0.1.0",
            "CFBundleVersion": "0.1.0", "CFBundleExecutable": "agentbaton-desktop",
        }))
        self.dmg_bytes = b"delivery test data"
        self.write_archive()
        self.identity = {
            "source_commit": COMMIT, "source_tree": TREE, "target": PROVENANCE.TARGET,
            "archive_sha256": PROVENANCE.file_digest(self.archive),
            "dmg_sha256": PROVENANCE.digest(self.dmg_bytes),
        }

    def put(self, name, content):
        path = self.volume / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(content)
        return path

    def write_archive(self, *, filename=DMG, record=None, checklist=CHECKLIST, checksum=None, duplicate=False):
        if record is None:
            record = f"commit={COMMIT}\narchitecture=arm64\nchannel={CHANNEL}\n"
        if checksum is None:
            checksum = f"{PROVENANCE.digest(self.dmg_bytes)}  ./{filename}\n"
        with zipfile.ZipFile(self.archive, "w", compression=zipfile.ZIP_DEFLATED) as archive:
            archive.writestr(filename, self.dmg_bytes)
            archive.writestr(PROVENANCE.BUILD_RECORD, record)
            archive.writestr(PROVENANCE.CHECKLIST, checklist)
            archive.writestr(PROVENANCE.CHECKSUMS, checksum)
            if duplicate:
                with warnings.catch_warnings():
                    warnings.simplefilter("ignore", UserWarning)
                    archive.writestr(PROVENANCE.CHECKSUMS, checksum)

    def unpack(self):
        return PROVENANCE.unpack_archive(self.archive, self.root, self.inputs)

    def inspect(self):
        with patch.object(PROVENANCE, "inspect_signature",
                          return_value={"type": "ad_hoc", "publisher_trust": "unknown"}):
            return PROVENANCE.inspect_volume(self.volume, self.inputs)

    def call_report(self, digest=None, receipt=None, receipt_sha=None):
        if digest is None:
            digest = PROVENANCE.file_digest(self.archive)
        args = SimpleNamespace(
            repo=ROOT, expected_commit=COMMIT, archive=self.archive, expected_archive_sha256=digest,
            receipt=receipt, receipt_sha256=receipt_sha,
        )
        with patch.object(PROVENANCE, "source_inputs", return_value=self.inputs), \
             patch.object(PROVENANCE, "mounted_dmg", return_value=nullcontext(self.volume)), \
             patch.object(PROVENANCE, "inspect_signature",
                          return_value={"type": "ad_hoc", "publisher_trust": "unknown"}):
            return PROVENANCE.report(args)

    def make_receipt(self, bindings, **overrides):
        receipt = {"schema_version": 1, **self.identity, "bindings": bindings, **overrides}
        path = self.root / "receipt.json"
        path.write_text(json.dumps(receipt))
        return path, PROVENANCE.file_digest(path)

    def linker_binding(self, row):
        image = self.volume / row["path"]
        linked = self.root / image.name
        linked.write_bytes(image.read_bytes())
        evidence = self.root / "native.map"
        evidence.write_text(
            f"# Path: /build/{linked.name}\n# Arch: arm64\n# Object files:\n"
            "[  0] linker synthesized\n[  1] /build/sample.o\n"
            "# Sections:\n0x100000000 0x10 __TEXT __text\n"
            "# Symbols:\n0x100000000 0x10 [  1] _main\n"
        )
        return {
            "kind": "linker", "file": row["path"], "file_sha256": row["sha256"],
            "record": evidence.name, "record_sha256": PROVENANCE.file_digest(evidence),
            "linked_output": linked.name,
        }

    def web_binding(self, row):
        asset = self.root / "chunk.js"
        asset.write_bytes(b"const fixture = 5;\n")
        records = self.root / "modules.json"
        records.write_text(json.dumps({
            "schema_version": 1, "chunk": asset.name, "chunk_sha256": PROVENANCE.file_digest(asset),
            "modules": [{"id": "src/fixture.js", "rendered_length": 14}],
        }))
        return {
            "kind": "web", "file": row["path"], "file_sha256": row["sha256"], "record": records.name,
            "record_sha256": PROVENANCE.file_digest(records), "asset": asset.name,
            "asset_sha256": PROVENANCE.file_digest(asset), "offset": 40,
        }

    def test_archive_integrity_and_git_resource_attribution(self):
        result = self.call_report()
        self.assertEqual(result["identity"], self.identity)
        rows = {item["path"]: item for item in result["files"]}
        self.assertEqual(rows[DESKTOP]["architecture"], "arm64")
        self.assertEqual(rows[DESKTOP]["static_linkage"], "unknown")
        self.assertEqual(rows[DESKTOP]["embedded_web"], "unknown")
        self.assertEqual(rows[PLIST]["attribution_state"], "unknown")
        self.assertEqual(rows[ICON]["attribution"][0]["kind"], "git_source_bytes")
        self.assertEqual(result["notarization"], "unknown")
        self.assertEqual(result["test_execution"], "unknown")

    def test_archived_bytes_cannot_drift_from_pinned_identity(self):
        expected = PROVENANCE.file_digest(self.archive)
        with self.archive.open("ab") as data:
            data.write(b"corruption")
        with self.assertRaisesRegex(PROVENANCE.EvidenceError, "archive SHA-256 mismatch"):
            self.call_report(digest=expected)

    def test_mixed_commit_names_and_duplicate_entries_are_rejected(self):
        for args in ({"filename": DMG.replace(COMMIT, "c" * 40)}, {"duplicate": True}):
            with self.subTest(args=args):
                self.write_archive(**args)
                with self.assertRaisesRegex(PROVENANCE.EvidenceError, "archive entries"):
                    self.unpack()
        self.write_archive(record=f"commit={'c' * 40}\narchitecture=arm64\nchannel={CHANNEL}\n")
        with self.assertRaisesRegex(PROVENANCE.EvidenceError, "build record commit mismatch"):
            self.unpack()

    def test_mixed_checklist_manifest_and_dmg_are_rejected(self):
        self.write_archive(checklist=b"other commit")
        with self.assertRaisesRegex(PROVENANCE.EvidenceError, "checklist does not match"):
            self.unpack()
        self.write_archive(checksum="invalid checksum line")
        with self.assertRaisesRegex(PROVENANCE.EvidenceError, "invalid development DMG checksum"):
            self.unpack()
        self.write_archive(checksum=f"{'f' * 64}  ./{DMG}\n")
        with self.assertRaisesRegex(PROVENANCE.EvidenceError, "development DMG SHA-256 mismatch"):
            self.unpack()

    def test_wrong_cpu_fat_binary_and_malformed_commands_are_rejected(self):
        for binary, expected in (
            (macho(cpu=0x01000007), "thin ARM64"),
            (b"\xca\xfe\xba\xbe" + b"\0" * 32, "thin ARM64"),
            (macho(command_size=32), "invalid Mach-O load command"),
        ):
            with self.subTest(expected=expected):
                self.put(CLI, binary)
                with self.assertRaisesRegex(PROVENANCE.EvidenceError, expected):
                    self.inspect()

    def test_invalid_bundle_shortcut_and_symlink_are_rejected(self):
        shortcut = self.volume / "Applications"
        shortcut.unlink()
        shortcut.symlink_to(self.root)
        with self.assertRaisesRegex(PROVENANCE.EvidenceError, "Applications shortcut"):
            self.inspect()
        shortcut.unlink()
        shortcut.symlink_to("/Applications")
        extra = self.volume / APP / "Resources" / "extra"
        extra.symlink_to(self.volume / ICON)
        with self.assertRaisesRegex(PROVENANCE.EvidenceError, "regular input file"):
            self.inspect()

    def test_receipt_rejects_mismatched_archive_dmg_source_tree_and_target(self):
        binding = self.linker_binding(self.inspect()[DESKTOP])
        for field in self.identity:
            with self.subTest(field=field):
                receipt, sha = self.make_receipt([binding], **{field: "other"})
                with self.assertRaisesRegex(PROVENANCE.EvidenceError, "receipt package identity mismatch"):
                    self.call_report(receipt=receipt, receipt_sha=sha)

    def test_pinned_receipts_bind_only_the_observed_carrier(self):
        rows = self.inspect()
        receipt, sha = self.make_receipt([
            self.linker_binding(rows[DESKTOP]), self.web_binding(rows[DESKTOP]),
        ])
        result = self.call_report(receipt=receipt, receipt_sha=sha)
        rows = {row["path"]: row for row in result["files"]}
        self.assertEqual(rows[DESKTOP]["static_linkage"], "bound_receipt")
        self.assertEqual(rows[DESKTOP]["embedded_web"], "bound_receipt")
        self.assertEqual(rows[DESKTOP]["attribution"][0]["object_inputs"], ["sample.o"])
        self.assertEqual(rows[DESKTOP]["attribution"][1]["modules"][0]["id"], "src/fixture.js")
        self.assertEqual(rows[DAEMON]["static_linkage"], "unknown")
        self.assertEqual(result["receipt_sha256"], sha)

    def test_receipt_digest_is_independent_and_web_bytes_are_checked(self):
        binding = self.web_binding(self.inspect()[DESKTOP])
        receipt, sha = self.make_receipt([binding])
        with self.assertRaisesRegex(PROVENANCE.EvidenceError, "externally pinned"):
            self.call_report(receipt=receipt)
        with self.assertRaisesRegex(PROVENANCE.EvidenceError, "sidecar SHA-256 mismatch"):
            self.call_report(receipt=receipt, receipt_sha="0" * 64)
        binding["offset"] -= 1
        receipt, sha = self.make_receipt([binding])
        with self.assertRaisesRegex(PROVENANCE.EvidenceError, "Web asset bytes do not match"):
            self.call_report(receipt=receipt, receipt_sha=sha)
        binding["file_sha256"] = "b" * 64
        receipt, sha = self.make_receipt([binding])
        with self.assertRaisesRegex(PROVENANCE.EvidenceError, "receipt bundle file SHA-256 mismatch"):
            self.call_report(receipt=receipt, receipt_sha=sha)

    def test_mismatched_link_map_name_and_unsafe_sidecar_are_rejected(self):
        binding = self.linker_binding(self.inspect()[DESKTOP])
        record = self.root / binding["record"]
        record.write_text(record.read_text().replace("agentbaton-desktop", "other"))
        binding["record_sha256"] = PROVENANCE.file_digest(record)
        receipt, sha = self.make_receipt([binding])
        with self.assertRaisesRegex(PROVENANCE.EvidenceError, "link map output name mismatch"):
            self.call_report(receipt=receipt, receipt_sha=sha)
        binding["record"] = "../outside.map"
        receipt, sha = self.make_receipt([binding])
        with self.assertRaisesRegex(PROVENANCE.EvidenceError, "invalid receipt sidecar filename"):
            self.call_report(receipt=receipt, receipt_sha=sha)

    def test_invalid_cli_input_does_not_emit_a_report(self):
        result = subprocess.run([
            sys.executable, str(SCRIPT), "--archive", str(self.archive),
            "--expected-archive-sha256", "a" * 64, "--expected-commit", "INVALID_PRIVATE_SENTINEL",
        ], capture_output=True, text=True, timeout=10)
        self.assertEqual(result.returncode, 1)
        self.assertEqual(result.stdout, "")
        self.assertIn("invalid Git commit", result.stderr)
        self.assertNotIn("INVALID_PRIVATE_SENTINEL", result.stderr)

    def test_build_commit_is_resolved_without_git_replace_objects(self):
        repo = self.root / "source"
        repo.mkdir()

        def git(*args):
            return subprocess.check_output(["git", "-C", str(repo), *args], text=True).strip()

        git("init", "-q")
        config = repo / "apps/desktop/src-tauri/tauri.conf.json"
        config.parent.mkdir(parents=True)
        icon = config.parent / "icons/icon.icns"
        icon.parent.mkdir()
        icon.write_bytes(b"trusted icon")
        checklist = repo / "docs/execution/DEVELOPMENT_ACCEPTANCE.md"
        checklist.parent.mkdir(parents=True)
        checklist.write_bytes(CHECKLIST)
        config.write_text(json.dumps({
            "productName": "AgentBaton", "identifier": "dev.agentbaton", "version": "0.1.0",
            "bundle": {"externalBin": ["binaries/batond", "binaries/baton"]},
        }))
        git("add", ".")
        identity = ["-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid",
                    "-c", "commit.gpgsign=false"]
        git(*identity, "commit", "-qm", "source commit")
        expected, tree = git("rev-parse", "HEAD", "HEAD^{tree}").splitlines()
        icon.write_bytes(b"replacement icon")
        git("add", ".")
        git(*identity, "commit", "-qm", "replacement commit")
        git("replace", expected, "HEAD")
        self.assertNotEqual(git("rev-parse", expected + "^{tree}"), tree)
        source = PROVENANCE.source_inputs(repo, expected)
        self.assertEqual(source["tree"], tree)
        self.assertEqual(source["repo_resources"][ICON]["sha256"], PROVENANCE.digest(b"trusted icon"))

    def test_ad_hoc_observation_does_not_upgrade_a_failed_verification(self):
        responses = [
            SimpleNamespace(returncode=0, stderr=b"Signature=adhoc\nTeamIdentifier=not set\n"),
            SimpleNamespace(returncode=1, stderr=b"fixture raw diagnostic"),
        ]
        with patch.object(PROVENANCE.subprocess, "run", side_effect=responses):
            result = PROVENANCE.inspect_signature(self.volume / DESKTOP)
        self.assertEqual(result["type"], "ad_hoc")
        self.assertEqual(result["verification"], "failed")
        self.assertEqual(result["verification_exit_code"], 1)
        self.assertEqual(result["publisher_trust"], "unknown")
        self.assertNotIn("raw diagnostic", json.dumps(result))


if __name__ == "__main__":
    unittest.main()
