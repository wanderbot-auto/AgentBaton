"""Test-only outer inverse for nine explicitly pinned schema18 source changes.

No source discovery, Git, execution, or wildcard rewriting at test time. Every
input byte is authenticated before positional edits; original inner pins remain.
"""
import hashlib
import json
from pathlib import Path

CHILD_PLANNING_PATH = 'apps/desktop/tests/factory-child-planning-real-ipc.py'
LAYERS = json.loads((Path(__file__).parent / 'fixtures/schema18_source_inverse.json').read_text())

# Admit the exact current projector, then reconstruct the previous source
# before running any of the unchanged Schema18 and inner inverse layers.
LAYERS['scripts/record-factory-rework-ci-evidence.py']['followup'] = {
    'after': '59654aacd1d57da3d7e5edb48841e1fd471c2eaadad923a8055c593266948f34',
    'edits': [{
        'start': 19, 'end': 20,
        'expected': 'SCHEMA = 19\n',
        'replacement': 'SCHEMA = 18\n',
    }],
}
LAYERS['tests/infrastructure/test_factory_rework_attempt_budget.py']['followup'] = {
    'after': '1b1eb64a5ebbd9675f96ff68c1448dfc3d6cc2dc98487ba1d50d329f6c99ce60',
    'edits': [{
        'start': 617, 'end': 618,
        'expected': "            if sql=='PRAGMA user_version':return Cursor([(19,)])\n",
        'replacement': "            if sql=='PRAGMA user_version':return Cursor([(18,)])\n",
    }],
}


def normalize_schema18_input(path, text, *, error=AssertionError):
    from tests.infrastructure.host19_child_goal_source_fixture import normalize_child_goal_outer_layer

    if path == 'tests/infrastructure/test_child_goal_diagnostics_v4.py':
        from tests.infrastructure.mantine_child_goal_source_fixture import restore_mantine_guard
        text = restore_mantine_guard(text, error=error)
    from tests.infrastructure.mantine_picker_source_fixture import restore_mantine_picker_source
    text = restore_mantine_picker_source(path, text.encode(), error=error).decode()
    return normalize_child_goal_outer_layer(path, text, error=error)


def strip_schema18_source(path, text, *, legacy=(), error=AssertionError):
    if type(path) is not str or path not in LAYERS or type(text) is not str:
        raise error('Unknown driver source: schema18 inverse arguments')
    text = normalize_schema18_input(path, text, error=error)
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
    text = normalize_schema18_input(CHILD_PLANNING_PATH, text)
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
