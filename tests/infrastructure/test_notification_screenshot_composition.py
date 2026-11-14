"""Exact two-parent source controls only; no browser/Core/Provider execution."""
import hashlib
from pathlib import Path
import unittest

from tests.infrastructure import integration_screenshot_failure_fixture as S
from tests.infrastructure import notification_screenshot_composition_fixture as C
from tests.infrastructure import test_integration_driver_preflight_diagnostics as P

ROOT = Path(__file__).resolve().parents[2]


class NotificationScreenshotComposition(unittest.TestCase):
    def variants(self):
        composed = (ROOT / 'apps/desktop/tests/integration-real-ipc.py').read_text()
        notification = C.strip_composed_screenshot_layer(composed)
        root = C.strip_composed_notification_layer(composed)
        original = S.strip_screenshot_driver_layer(root)
        return composed, notification, root, original

    def test_all_four_exact_variants_preserve_both_original_inverse_paths(self):
        composed, notification, root, original = self.variants()
        for text, digest in [(composed, C.NOTIFICATION_AFTER_SHA256), (notification, C.NOTIFICATION_BEFORE_SHA256),
                             (root, S.DRIVER_AFTER_SHA256), (original, S.DRIVER_BEFORE_SHA256)]:
            self.assertEqual(hashlib.sha256(text.encode()).hexdigest(), digest)
        self.assertEqual(C.strip_composed_notification_layer(notification), original)
        self.assertEqual(C.strip_composed_screenshot_layer(root), original)
        self.assertEqual(C.strip_composed_notification_layer(root), root)
        self.assertEqual(C.strip_composed_screenshot_layer(notification), notification)
        for strip in (C.strip_composed_notification_layer, C.strip_composed_screenshot_layer):
            self.assertEqual(strip(original), original)
        self.assertIn("phase = 'notification_probe'", composed)
        self.assertIn("report['active_screenshot']", composed)

    def test_all_four_variants_still_restore_the_entire_preflight_baseline(self):
        controls = P.ExactOriginalSourceControls()
        for text in self.variants():
            with self.subTest(source=hashlib.sha256(text.encode()).hexdigest()):
                restored = controls.restore_driver(text)
                self.assertEqual(hashlib.sha256(restored.encode()).hexdigest(),
                    '1e34768c2d79314163080300e741ed3a98a9ba64752f0b38152852b15028bd1e')

    def test_unknown_or_removed_duplicate_moved_changed_layer_and_original_guards_refuse(self):
        for text in self.variants():
            changes = [text + '\nUNKNOWN_UNREVIEWED_BYTE = True\n',
                text.replace('MAX_CHROMIUM_IMAGE = 536870912', 'MAX_CHROMIUM_IMAGE = 1073741824', 1),
                text.replace('MAX_BODY, MAX_REPLY, MAX_REVIEW = 270336, 4194304, 1048576',
                             'MAX_BODY, MAX_REPLY, MAX_REVIEW = 270336, 4194304, 2097152', 1)]
            for old, new in C.NOTIFICATION_EDITS:
                if new in text:
                    changes.extend([text.replace(new, '', 1), text.replace(new, new * 2, 1),
                                    text.replace(new, new.replace('\n', '\r\n'), 1)])
                    if text.count('def reservation(') == 1 and not new.startswith('def reservation('):
                        removed = text.replace(new, '', 1)
                        changes.append(removed.replace('def reservation(', new + 'def reservation(', 1))
            for old, new in S.DRIVER_EDITS:
                if new in text:
                    changes.extend([text.replace(new, '', 1), text.replace(new, new * 2, 1)])
            for changed in changes:
                self.assertNotEqual(changed, text)
                for strip in (C.strip_composed_notification_layer, C.strip_composed_screenshot_layer):
                    with self.subTest(source=hashlib.sha256(text.encode()).hexdigest()), self.assertRaises(AssertionError):
                        strip(changed)


if __name__ == '__main__': unittest.main()
