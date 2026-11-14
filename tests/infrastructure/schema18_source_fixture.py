"""Test-only outer inverse for nine explicitly pinned schema18 source changes.

No source discovery, Git, execution, or wildcard rewriting at test time. Every
input byte is authenticated before positional edits; original inner pins remain.
"""
import hashlib
import json
from pathlib import Path

LAYERS = json.loads((Path(__file__).parent / 'fixtures/schema18_source_inverse.json').read_text())


def strip_schema18_source(path, text, *, legacy=(), error=AssertionError):
    if type(path) is not str or path not in LAYERS or type(text) is not str:
        raise error('Unknown driver source: schema18 inverse arguments')
    layer = LAYERS[path]
    digest = hashlib.sha256(text.encode()).hexdigest()
    if digest == layer['before'] or digest in legacy:
        return text
    followup = layer.get('followup')
    if followup is not None and digest == followup['after']:
        text = restore(text, followup['edits'], layer['after'], error)
        digest = layer['after']
    if digest != layer['after']:
        raise error('Unknown driver source: complete schema18 input changed')
    return restore(text, layer['edits'], layer['before'], error)


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
