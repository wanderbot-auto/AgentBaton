"""Restore the exact pre-Store19 WebView runner and shell for historical pins."""
from hashlib import sha256


_LAYERS = {
    'scripts/native-run.py': (
        '12ae8dbc28abef5fbdd0031fffbb5d45d9ae78c724242abdf747432cc201e503',
        '205d76e38721760744bd4d889e34ab3d2cf436ebc94583fee40fa17029ffac0a',
        (
            (b'READONLY_SCHEMA_VERSION = 19\nWEBCRYPTO_INPUT = ', b'WEBCRYPTO_INPUT = ', 1),
            (b"counts['schema_version'] != READONLY_SCHEMA_VERSION",
             b"counts['schema_version'] != 18", 2),
            (b"return f'native Factory schema{READONLY_SCHEMA_VERSION} or one of twelve independent zero counts is invalid'",
             b"return 'native Factory schema18 or one of twelve independent zero counts is invalid'", 1),
            (b'Current readonly metadata only, never an integration approval/run receipt.',
             b'Fixed18 readonly metadata only, never an integration approval/run receipt.', 1),
            (b"return f'native integration schema{READONLY_SCHEMA_VERSION} or one of four independent zero counts is invalid'",
             b"return 'native integration schema18 or one of four independent zero counts is invalid'", 1),
            (b"start['schema_version'] != READONLY_SCHEMA_VERSION or finish['schema_version'] != READONLY_SCHEMA_VERSION",
             b"start['schema_version'] != 18 or finish['schema_version'] != 18", 1),
            (b"graph_before['schema_version'] != READONLY_SCHEMA_VERSION or graph_after['schema_version'] != READONLY_SCHEMA_VERSION",
             b"graph_before['schema_version'] != 18 or graph_after['schema_version'] != 18", 1),
        ),
    ),
    'scripts/native-macos-smoke.sh': (
        'd4b27ec4ff8c2ffeec50d70ade13657972c9abd3c5a094aff086a78d9b89b00f',
        '79841c8b178ed3a68a6a62d13ccc81581e43798be3c1c29a00bf161a8b3dfefd',
        (
            (b"['schema_version'] == native_runner.READONLY_SCHEMA_VERSION",
             b"['schema_version'] == 18", 3),
        ),
    ),
}
NATIVE_WEBVIEW_PATHS = frozenset(_LAYERS)
_NATIVE_RUNNER = 'scripts/native-run.py'
_DARWIN_CLEANUP_LAYER = (
    'aa8930e9aeefb72811c599e7d8ee6e781fee53c8bf9f8b428173e387ca17eca8',
    _LAYERS[_NATIVE_RUNNER][0],
    (
        (b'        timed_out = False\n        observation_error = None\n',
         b'        timed_out = False\n        exited = False\n        observation_error = None\n', 1),
        (b'            while True:\n                if owned_child_exited(child.pid):\n',
         b'            while True:\n                exited = owned_child_exited(child.pid)\n'
         b'                if exited:\n', 1),
        (b'                        # Recheck the original group while its leader is still unreaped.\n'
         b'                        quiescent = (error.errno == errno.EPERM\n',
         b'                        quiescent = (error.errno == errno.EPERM and exited\n', 1),
    ),
)


def _restore(source, layer):
    current, previous, edits = layer
    if sha256(source).hexdigest() != current:
        raise ValueError('Unknown complete Store19 WebView source')
    for new, old, count in edits:
        if source.count(new) != count:
            raise ValueError('WebView source anchor changed')
        source = source.replace(new, old, count)
    if sha256(source).hexdigest() != previous:
        raise ValueError('WebView inverse did not restore the exact historical source')
    return source


def restore_native_runner_before_darwin_cleanup(source: bytes) -> bytes:
    """Peel the cleanup change back to the pinned Store19 runner."""
    if type(source) is not bytes:
        raise ValueError('Non-byte native runner input')
    return _restore(source, _DARWIN_CLEANUP_LAYER)


def restore_native_webview_before_schema19(path: str, source: bytes) -> bytes:
    """Peel only the reviewed current files and require the complete old bytes."""
    if path not in _LAYERS or type(source) is not bytes:
        raise ValueError('Unknown WebView source path or non-byte input')
    if path == _NATIVE_RUNNER:
        source = restore_native_runner_before_darwin_cleanup(source)
    return _restore(source, _LAYERS[path])
