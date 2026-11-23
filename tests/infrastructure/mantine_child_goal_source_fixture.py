"""Authenticate the Mantine-only layer before the immutable child Goal source inverses."""
import re

from tests.infrastructure.host19_child_goal_source_fixture import reverse_pinned_spans, sha


RENDERER_LAYER = (
    'bfa9a9494ff9bc1bd2743f4f00709af49e269befc72bc4008d6936d59a4408da',
    'c619626fa843ae4513b89f29ec1e6d6fd78827127b26150fc95f124eecf8c573',
)

GUARD_LAYER = (
    '74baefa24a591c78d0c40c465cbafd967d72166751188e6b56b22285073b1568',
    'c681f130f35f45dfaac8c57e1090a213c18539e9b7cf004ddfc8a8ca4af6a1ec',
    [
        (11, 12, 'b7f476ce8f3f5a0cca53ad1bc8d3055328393fe30edf43f357852238f2a8224a',
         ''),
        (58, 59, 'e215eac395f03cb9df097fbbf8a27344976dd25d7ab3e841219268ab368afc14',
         ''),
        (377, 378, '277634498a2423b67a9223aac7f0eeb0c5397a530f06e2961f81ca7e1ef7e4bc',
         '        self.assertEqual(source_sha(text),CONTEXT_SERIALIZATION_UI_SHA256)\n'),
    ],
)


def restore_mantine_renderer(source):
    candidate, baseline = RENDERER_LAYER
    if sha(source) != candidate:
        return source  # Older inputs and unknown changes still face the historical pins.
    source = source.removeprefix("import { Button, Code, Disclosure, DisclosureLabel } from './ui';\n")
    for component, tag in [('Button', 'button'), ('Disclosure', 'details'), ('DisclosureLabel', 'summary')]:
        source = re.sub(r'(</?)' + component + r'(?=[\s>])', r'\g<1>' + tag, source)
    source = source.replace('<Code block', '<pre').replace('</Code>', '</pre>')
    if sha(source) != baseline:
        raise AssertionError('Mantine child Goal inverse did not restore the reviewed renderer')
    return source


def restore_mantine_guard(source, *, error=AssertionError):
    if sha(source) != GUARD_LAYER[0]:
        return source
    return reverse_pinned_spans(source, GUARD_LAYER, error)
