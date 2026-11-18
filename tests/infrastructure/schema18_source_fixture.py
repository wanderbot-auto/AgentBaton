"""Test-only outer inverse for nine explicitly pinned schema18 source changes.

No source discovery, Git, execution, or wildcard rewriting at test time. Every
input byte is authenticated before positional edits; original inner pins remain.
"""
import hashlib
import json
from pathlib import Path

CHILD_PLANNING_PATH = 'apps/desktop/tests/factory-child-planning-real-ipc.py'
LAYERS = json.loads((Path(__file__).parent / 'fixtures/schema18_source_inverse.json').read_text())


def strip_schema18_source(path, text, *, legacy=(), error=AssertionError):
    if type(path) is not str or path not in LAYERS or type(text) is not str:
        raise error('Unknown driver source: schema18 inverse arguments')
    from tests.infrastructure.host19_child_goal_source_fixture import normalize_child_goal_outer_layer

    text = normalize_child_goal_outer_layer(path, text, error=error)
    layer = LAYERS[path]
    digest = hashlib.sha256(text.encode()).hexdigest()
    if digest == layer['before'] or digest in legacy:
        return text
    if path == 'apps/desktop/tests/integration-real-ipc.py':
        from tests.infrastructure import notification_screenshot_composition_fixture as notification
        if digest in (notification.CURRENT_COMPOSED_SHA256, notification.PRE_REFOCUS_SHA256):
            try:
                text = notification.strip_current_notification_layer(text)
            except AssertionError as exc:
                raise error(str(exc)) from exc
            digest = notification.CURRENT_WITHOUT_NOTIFICATION_SHA256
    historical = layer.get('historical_followup') if path == CHILD_PLANNING_PATH else None
    if historical is not None and digest == historical['after']:
        text = restore(text, historical['edits'], layer['after'], error)
        digest = layer['after']
    followup = layer.get('followup')
    if followup is not None and digest == followup['after']:
        text = restore(text, followup['edits'], layer['after'], error)
        digest = layer['after']
    if digest != layer['after']:
        raise error('Unknown driver source: complete schema18 input changed')
    return restore(text, layer['edits'], layer['before'], error)


def child_planning_before_schema19(text):
    """Verify the active layer and recompose the exact historical followup."""
    layer = LAYERS[CHILD_PLANNING_PATH]
    historical = layer['historical_followup']
    assert type(text) is str, 'Unknown driver source: Child Planning input'
    digest = hashlib.sha256(text.encode()).hexdigest()
    if digest == historical['after']:
        restore(text, historical['edits'], layer['after'], AssertionError)
        return text
    active = layer['followup']
    assert digest == active['after'], 'Unknown driver source: current Child Planning input changed'
    parent = restore(text, active['edits'], layer['after'], AssertionError)
    lines = parent.splitlines(keepends=True)
    # The old offsets are in the followup bytes. Applying spans in ascending
    # order incorporates the preceding offset shifts before each next span.
    for edit in historical['edits']:
        start = edit['start']
        original = edit['replacement'].splitlines(keepends=True)
        assert type(start) is int and 0 <= start <= len(lines) - len(original), 'Historical span bounds changed'
        assert lines[start:start + len(original)] == original, 'Historical parent span changed'
        inserted = edit['expected'].splitlines(keepends=True)
        assert start + len(inserted) == edit['end'], 'Historical followup span length changed'
        lines[start:start + len(original)] = inserted
    result = ''.join(lines)
    assert hashlib.sha256(result.encode()).hexdigest() == historical['after'], 'Historical followup bytes changed'
    return result


def restore(text, edits, expected, error):
    lines = text.splitlines(keepends=True)
    for edit in reversed(edits):
        start, end = edit['start'], edit['end']
        if type(start) is not int or type(end) is not int or not 0 <= start < end <= len(lines):
            raise error('Schema18 inverse bounds changed')
        if ''.join(lines[start:end]) != edit['expected']:
            raise error('Schema18 inverse exact source span changed')
        lines[start:end] = edit['replacement'].splitlines(keepends=True)
    restored = ''.join(lines)
    if hashlib.sha256(restored.encode()).hexdigest() != expected:
        raise error('Schema18 inverse did not restore every original byte')
    return restored
