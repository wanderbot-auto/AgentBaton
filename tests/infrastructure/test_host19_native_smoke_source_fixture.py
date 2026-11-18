"""Bind the schema19 smoke source to both reviewed historical byte layers."""
import hashlib
import json
from pathlib import Path
import unittest

from tests.infrastructure.host19_native_smoke_source_fixture import (
    NATIVE_SMOKE_PATH,
    SOURCE_EDITS,
    restore_native_smoke_before_schema19,
)
from tests.infrastructure.schema18_source_fixture import restore

ROOT = Path(__file__).resolve().parents[2]
PARENT_SHA256 = "5145fdce52955077dcfd591b712977452dec3cfd09bb4bb06807968e0a1bcdfa"
HISTORICAL_SHA256 = "95da64173f62be210d35fd626006f76ff33cda5ce96b333517c1244d23d2a4e0"


class Host19NativeSmokeSourceFixture(unittest.TestCase):
    def setUp(self):
        self.source = ROOT / NATIVE_SMOKE_PATH
        self.raw = self.source.read_bytes()

    def test_restores_the_exact_parent_and_the_older_pinned_blob(self):
        parent = restore_native_smoke_before_schema19(NATIVE_SMOKE_PATH, self.raw)
        self.assertEqual(hashlib.sha256(parent).hexdigest(), PARENT_SHA256)
        saved_layers = Path(__file__).parent / "fixtures" / "native_acceptance_followup.json"
        layer = json.loads(saved_layers.read_bytes())[NATIVE_SMOKE_PATH]
        self.assertEqual(layer["after"], PARENT_SHA256)
        historical = restore(parent.decode(), layer["edits"], HISTORICAL_SHA256, AssertionError)
        self.assertEqual(hashlib.sha256(historical.encode()).hexdigest(), HISTORICAL_SHA256)
        self.assertEqual(self.source.read_bytes(), self.raw)

    def test_refuses_bom_crlf_and_the_reviewed_parent_as_input(self):
        parent = restore_native_smoke_before_schema19(NATIVE_SMOKE_PATH, self.raw)
        invalid = {
            "BOM": b"\xef\xbb\xbf" + self.raw,
            "CRLF": self.raw.replace(b"\n", b"\r\n"),
            "parent": parent,
            "appended byte": self.raw + b"\n",
        }
        for variant, raw in invalid.items():
            with self.subTest(variant=variant), self.assertRaises(ValueError):
                restore_native_smoke_before_schema19(NATIVE_SMOKE_PATH, raw)

    def test_refuses_changed_missing_or_duplicated_source_spans(self):
        leading, current, _, trailing = SOURCE_EDITS[0]
        anchor = leading + current + trailing
        self.assertEqual(self.raw.count(anchor), 1)
        invalid = {
            "changed guard": self.raw.replace(anchor, leading + b" " + current + trailing, 1),
            "missing span": self.raw.replace(anchor, b"", 1),
            "duplicated span": self.raw + anchor,
        }
        for variant, raw in invalid.items():
            with self.subTest(variant=variant), self.assertRaises(ValueError):
                restore_native_smoke_before_schema19(NATIVE_SMOKE_PATH, raw)

    def test_refuses_other_files_and_noncanonical_paths(self):
        invalid = (
            "apps/desktop/src-tauri/src/native_parent.rs",
            "./" + NATIVE_SMOKE_PATH,
            str(self.source),
            Path(NATIVE_SMOKE_PATH),
        )
        for path in invalid:
            with self.subTest(path=path), self.assertRaises(ValueError):
                restore_native_smoke_before_schema19(path, self.raw)

    def test_refuses_nonbyte_source(self):
        for value in (self.raw.decode(), bytearray(self.raw), None):
            with self.subTest(value_type=type(value)), self.assertRaises(ValueError):
                restore_native_smoke_before_schema19(NATIVE_SMOKE_PATH, value)
