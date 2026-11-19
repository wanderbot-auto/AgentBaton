"""Exact reviewed source restoration and refusal controls for Owner schema19."""
import hashlib
import json
from pathlib import Path
import unittest

from tests.infrastructure.host19_native_integration_owner_source_fixture import (
    GRANT_FACTORY_STORE19_LAYER,
    SOURCE_LAYERS,
    restore_grant_factory_owner_before_store19,
    restore_integration_owner_before_schema19,
)
from tests.infrastructure.schema18_source_fixture import restore

ROOT = Path(__file__).resolve().parents[2]
PYTHON = "scripts/native-owner-dialog-smoke.py"
OLDER_PYTHON_SHA = "e2ca962c8e5d8af29bc04bb477c6d9d4fc876b7143b33ffafca8a730944fad94"


def digest(raw):
    return hashlib.sha256(raw).hexdigest()


class NativeIntegrationOwnerSourceFixture(unittest.TestCase):
    def test_all_current_inputs_restore_the_fixed_full_predecessor(self):
        for path, (_, parent_sha, edits) in SOURCE_LAYERS.items():
            with self.subTest(path=path):
                source = ROOT / path
                raw = source.read_bytes()
                integration_source = (restore_grant_factory_owner_before_store19(raw)
                                      if path == PYTHON else raw)
                for leading, current, _, trailing in edits:
                    self.assertEqual(integration_source.count(leading + current + trailing), 1)
                parent = restore_integration_owner_before_schema19(path, raw)
                self.assertEqual(digest(parent), parent_sha)
                self.assertEqual(source.read_bytes(), raw)

    def test_grant_factory_store19_restores_the_exact_pinned_integration_input(self):
        raw = (ROOT / PYTHON).read_bytes()
        current_sha, parent_sha, edits = GRANT_FACTORY_STORE19_LAYER
        self.assertEqual(digest(raw), current_sha)
        self.assertEqual(parent_sha, SOURCE_LAYERS[PYTHON][0])
        integration_source = restore_grant_factory_owner_before_store19(raw)
        self.assertEqual(digest(integration_source), parent_sha)
        for leading, current, predecessor, trailing in edits:
            anchor = leading + current + trailing
            self.assertEqual(raw.count(anchor), 1)
            self.assertEqual(integration_source.count(leading + predecessor + trailing), 1)
            for changed in (raw + anchor, raw.replace(anchor, b'', 1)):
                with self.subTest(anchor=current, changed_size=len(changed)), self.assertRaises(ValueError):
                    restore_grant_factory_owner_before_store19(changed)
        with self.assertRaises(ValueError):
            restore_grant_factory_owner_before_store19(integration_source)
        with self.assertRaises(ValueError):
            restore_integration_owner_before_schema19(PYTHON, integration_source)

    def test_old_native_python_inverse_still_recovers_original_pin(self):
        legacy = (Path(__file__).parent / 'fixtures/native_acceptance_followup.json').read_bytes()
        self.assertEqual(digest(legacy), "4c676e3f72912687d30844afcee5653c9420106ffe43d7cf33babb09b19d1f0a")
        layer = json.loads(legacy)[PYTHON]
        parent = restore_integration_owner_before_schema19(PYTHON, (ROOT / PYTHON).read_bytes())
        self.assertEqual(digest(parent), layer["after"])
        original = restore(parent.decode(), layer["edits"], OLDER_PYTHON_SHA, AssertionError)
        self.assertEqual(digest(original.encode()), OLDER_PYTHON_SHA)

    def test_unknown_inputs_and_partial_or_duplicated_anchors_refuse(self):
        for path, (_, _, edits) in SOURCE_LAYERS.items():
            raw = (ROOT / path).read_bytes()
            parent = restore_integration_owner_before_schema19(path, raw)
            leading, current, _, trailing = edits[0]
            anchor = leading + current + trailing
            controls = (
                parent, b"\xef\xbb\xbf" + raw, raw.replace(b"\n", b"\r\n"), raw + b"\n",
                raw.replace(anchor, b"", 1), raw + anchor,
                raw.replace(anchor, leading + b" " + current + trailing, 1),
                bytearray(raw), raw.decode(),
            )
            for invalid in controls:
                with self.subTest(path=path, kind=type(invalid)), self.assertRaises(ValueError):
                    restore_integration_owner_before_schema19(path, invalid)

    def test_path_is_exact_and_paired_to_its_own_current_bytes(self):
        raw = (ROOT / PYTHON).read_bytes()
        others = [path for path in SOURCE_LAYERS if path != PYTHON]
        for path in ("./" + PYTHON, str(ROOT / PYTHON), Path(PYTHON), "scripts/other.py", *others):
            with self.subTest(path=path), self.assertRaises(ValueError):
                restore_integration_owner_before_schema19(path, raw)
