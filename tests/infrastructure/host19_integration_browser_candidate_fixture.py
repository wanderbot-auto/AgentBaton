"""Exact test-only outer inverse for the candidate schema19 manifest guard."""
import hashlib

DRIVER_PATH = "apps/desktop/tests/integration-real-ipc.py"
CURRENT_SHA256 = "d7d163f945431abe328922c3f6e378c7665d3d37b9f8accbdc2f8a04454526be"
PARENT_SHA256 = "e5036fd15afa431228a9b11553b0602e144636dda58fecf4fbbbcaac8633f6f3"
CURRENT_GUARD = (
    b"    driver_preflight_stage(diagnostic, 'candidate_schema_guard')\n"
    b"    require(type(manifest.get('schema')) is int and manifest['schema'] == 19)\n"
    b"    driver_preflight_stage(diagnostic, 'candidate_source_guard')\n"
)
PARENT_GUARD = (
    b"    driver_preflight_stage(diagnostic, 'candidate_schema_guard')\n"
    b"    require(type(manifest.get('schema')) is int and manifest['schema'] == 18)\n"
    b"    driver_preflight_stage(diagnostic, 'candidate_source_guard')\n"
)


def restore_integration_browser_before_candidate19(path, raw):
    """Return exactly the reviewed predecessor for the pinned current driver."""
    if type(path) is not str or path != DRIVER_PATH or type(raw) is not bytes:
        raise ValueError("Expected raw bytes at the canonical Integration browser driver path")
    if hashlib.sha256(raw).hexdigest() != CURRENT_SHA256:
        raise ValueError("Current Integration browser candidate source has changed")
    if raw.count(CURRENT_GUARD) != 1:
        raise ValueError("Expected exactly one complete Integration browser candidate guard")
    parent = raw.replace(CURRENT_GUARD, PARENT_GUARD, 1)
    if hashlib.sha256(parent).hexdigest() != PARENT_SHA256:
        raise ValueError("Integration browser candidate inverse did not restore the pinned parent")
    return parent
