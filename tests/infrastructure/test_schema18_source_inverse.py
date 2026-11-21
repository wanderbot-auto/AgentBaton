"""Exact current-to-historical source restoration and tamper controls only."""
import copy
import hashlib
from pathlib import Path
import unittest
from unittest import mock
from tests.infrastructure import schema18_source_fixture as F
from tests.infrastructure import notification_screenshot_composition_fixture as N

from tests.infrastructure.host19_child_goal_source_fixture import normalize_child_goal_outer_layer
from tests.infrastructure.host19_browser_ci_projector_source_fixture import (
    PROJECTOR_PATH,
    restore_projector_before_schema19,
)
from tests.infrastructure.host19_integration_browser_source_fixture import (
    DRIVER_PATH,
    read_historical_driver,
)

ROOT = Path(__file__).resolve().parents[2]


def input_source(path):
    if path == DRIVER_PATH:
        return read_historical_driver(ROOT)
    if path == PROJECTOR_PATH:
        return restore_projector_before_schema19(path, (ROOT / path).read_bytes()).decode()
    return normalize_child_goal_outer_layer(path, (ROOT / path).read_text())


class Schema18SourceInverse(unittest.TestCase):
    def test_every_complete_current_source_restores_exact_original(self):
        for path, layer in F.LAYERS.items():
            with self.subTest(path=path):
                current = input_source(path)
                current_layer = layer.get('followup', layer)
                if path == 'apps/desktop/tests/integration-real-ipc.py':
                    uncomposed = N.strip_current_notification_layer(current)
                else:
                    uncomposed = current
                self.assertEqual(hashlib.sha256(uncomposed.encode()).hexdigest(), current_layer['after'])
                restored = F.strip_schema18_source(path, current)
                self.assertEqual(hashlib.sha256(restored.encode()).hexdigest(), layer['before'])
                self.assertEqual(F.strip_schema18_source(path, restored), restored)

    def test_unknown_partial_duplicate_moved_and_outside_changes_refuse(self):
        for path, layer in F.LAYERS.items():
            text = input_source(path)
            mutations = [text + '# unreviewed outside change\n']
            if path == 'apps/desktop/tests/integration-real-ipc.py':
                text = N.strip_current_notification_layer(text)
            for edit in layer.get('followup', layer)['edits']:
                start, end = edit['start'], edit['end']
                lines = text.splitlines(keepends=True)
                mutations += [
                    ''.join(lines[:start]) + edit['replacement'] + ''.join(lines[end:]),
                    text + edit['expected'],
                    ''.join(lines[:start] + lines[end:]) + edit['expected'],
                ]
            for changed in mutations:
                if hashlib.sha256(changed.encode()).hexdigest() in {layer['before'], layer['after']}:
                    continue  # Only complete, pinned historical sources are supported.
                with self.subTest(path=path), self.assertRaises(AssertionError):
                    F.strip_schema18_source(path, changed)

    def test_changed_manifest_anchor_bounds_or_output_pin_refuse(self):
        for path, layer in F.LAYERS.items():
            text = input_source(path)
            for mutation in ('after', 'before', 'expected', 'replacement', 'start'):
                changed = copy.deepcopy(layer)
                if mutation in ('after', 'before'):
                    changed[mutation] = '0' * 64
                elif mutation == 'start':
                    changed['edits'][0]['start'] = -1
                else:
                    changed['edits'][0][mutation] += '# changed\n'
                with self.subTest(path=path,mutation=mutation), mock.patch.dict(F.LAYERS,{path:changed}), self.assertRaises(AssertionError):
                    F.strip_schema18_source(path,text)
