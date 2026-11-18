"""Test-only inverse for the two reviewed schema19 driver guards."""
import hashlib

from tests.infrastructure.host19_browser_ci_projector_source_fixture import (
    restore_driver_before_labels19,
)
from tests.infrastructure.host19_integration_browser_candidate_fixture import (
    restore_integration_browser_before_candidate19,
)

DRIVER_PATH = "apps/desktop/tests/integration-real-ipc.py"
CURRENT_SHA256 = "e5036fd15afa431228a9b11553b0602e144636dda58fecf4fbbbcaac8633f6f3"
PARENT_SHA256 = "0131bee1842f3856507def3f41d4d45f0a9b38511167886461fc96d8a32d37fe"
GUARD_EDITS = (
    (
        b"        db.execute('PRAGMA query_only=ON'); db.execute('BEGIN')\n"
        b"        version = db.execute('PRAGMA user_version').fetchone()\n"
        b"        require(version == (19,) and type(version[0]) is int)\n"
        b"        yield db\n",
        b"        db.execute('PRAGMA query_only=ON'); db.execute('BEGIN')\n"
        b"        require(db.execute('PRAGMA user_version').fetchone() == (18,))\n"
        b"        yield db\n",
    ),
    (
        b"    require(shell.stat().st_nlink == 1)\n"
        b"    version = host.result('system.snapshot', {})['database_schema_version']\n"
        b"    require(type(version) is int and version == 19)\n",
        b"    require(shell.stat().st_nlink == 1)\n"
        b"    require(host.result('system.snapshot', {})['database_schema_version'] == 18)\n",
    ),
)


def restore_integration_browser_before_schema19(path, raw):
    """Return the previous driver bytes only for the pinned candidate and path."""
    if type(path) is not str or path != DRIVER_PATH or type(raw) is not bytes:
        raise ValueError("Expected raw bytes at the canonical Integration browser driver path")
    if hashlib.sha256(raw).hexdigest() != CURRENT_SHA256:
        raise ValueError("Current Integration browser driver source has changed")
    for current, parent in GUARD_EDITS:
        if raw.count(current) != 1:
            raise ValueError("Expected exactly one complete Integration browser guard")
        raw = raw.replace(current, parent, 1)
    if hashlib.sha256(raw).hexdigest() != PARENT_SHA256:
        raise ValueError("Integration browser driver inverse did not restore the pinned parent")
    return raw


def read_historical_driver(root):
    """Feed the exact pre-schema19 driver to the existing historical tests."""
    current = (root / DRIVER_PATH).read_bytes()
    prior_labels = restore_driver_before_labels19(DRIVER_PATH, current)
    parent = restore_integration_browser_before_candidate19(DRIVER_PATH, prior_labels)
    return restore_integration_browser_before_schema19(DRIVER_PATH, parent).decode()
