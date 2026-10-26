"""Design validation must cover authored docs, not installed package docs."""
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]


class DesignDiscoveryTests(unittest.TestCase):
    def test_ignored_dependency_docs_do_not_hide_authored_broken_links(self):
        with tempfile.TemporaryDirectory(dir=ROOT) as temporary:
            fixture = Path(temporary)
            dependency = fixture / 'node_modules' / 'package'
            dependency.mkdir(parents=True)
            (dependency / 'README.md').write_text('[omitted package docs](missing.md)')
            command = [sys.executable, str(ROOT / 'scripts/validate_design.py')]
            passing = subprocess.run(command, cwd=ROOT, capture_output=True, text=True)
            self.assertEqual(passing.returncode, 0, passing.stdout + passing.stderr)
            (fixture / 'authored.md').write_text('[broken authored docs](missing.md)')
            failing = subprocess.run(command, cwd=ROOT, capture_output=True, text=True)
            self.assertNotEqual(failing.returncode, 0)
            self.assertIn('authored.md', failing.stderr)
            self.assertIn('Broken link', failing.stderr)
