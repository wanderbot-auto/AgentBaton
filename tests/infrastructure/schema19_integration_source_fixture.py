"""Exact, read-only source restoration for the two reviewed schema19 fixtures."""
from hashlib import sha256
from pathlib import Path

IPC_PATH = "crates/core/tests/integration_ipc.rs"
RUNTIME_PATH = "crates/core/tests/fixtures/integration_runtime.rs"
_MAX_SOURCE_BYTES = 1_048_576

_IPC_VERSION = (
    b'        let version: u32 = db\n'
    b'            .pragma_query_value(None, "user_version", |r| r.get(0))\n'
    b'            .unwrap();\n'
    b'        assert_eq!(version, %d);\n'
)
_RUNTIME_SNAPSHOT = (
    b'        let server = Server::start(root.join("runtime"));\n'
    b'        assert_eq!(\n'
    b'            rpc(&server, "system.snapshot", json!({}), None, None)["database_schema_version"],\n'
    b'            %d\n'
    b'        );\n'
)
_RUNTIME_VERSION = (
    b'    let version: i64 = db\n'
    b'        .query_row("PRAGMA user_version", [], |r| r.get(0))\n'
    b'        .unwrap();\n'
    b'    assert_eq!(version, %d);\n'
)

# Input digests bind the entire source at reviewed reader commit 56c56e9e.
# Predecessor digests bind a076fc73 before the three current-schema changes.
_SOURCES = {
    IPC_PATH: (
        "b46d41dfe615706c6cbb113eee65370a4b08b68ddcc255a875eed34f33973a21",
        "58f68b35230b06b2d64f4143fcadeb1d075d2e77c8763c1007e666e738da4199",
        (_IPC_VERSION,),
    ),
    RUNTIME_PATH: (
        "54f86df36fd003889f342e2a3ce08418666370d4f89a817f779e362e07752cd4",
        "bf9a348a6923432c0ecb66d5fefbe1b40aee9d46729eddddcd62e04079d3d504",
        (_RUNTIME_SNAPSHOT, _RUNTIME_VERSION),
    ),
}


def restore_schema18_source(path: str, source: bytes) -> bytes:
    """Restore only the pinned current schema19 source."""
    if type(path) is not str or path not in _SOURCES:
        raise AssertionError("Unknown Integration source path")
    if type(source) is not bytes or not 0 < len(source) <= _MAX_SOURCE_BYTES:
        raise AssertionError("Integration source must be bounded raw bytes")

    current_sha, predecessor_sha, anchors = _SOURCES[path]
    if sha256(source).hexdigest() != current_sha:
        raise AssertionError("Unknown exact schema19 Integration source")

    restored = source
    for anchor in anchors:
        current = anchor % 19
        if restored.count(current) != 1:
            raise AssertionError("Unique schema19 Integration anchor required")
        restored = restored.replace(current, anchor % 18, 1)
    if sha256(restored).hexdigest() != predecessor_sha:
        raise AssertionError("Exact schema18 Integration source not restored")
    return restored


def read_schema18_source(root: Path, source: Path) -> str:
    """Read one repository fixture as bytes before handing text to the old chain."""
    try:
        path = source.relative_to(root).as_posix()
    except ValueError as error:
        raise AssertionError("Unknown Integration source path") from error
    if path not in _SOURCES:
        raise AssertionError("Unknown Integration source path")
    with source.open("rb") as stream:
        raw = stream.read(_MAX_SOURCE_BYTES + 1)
    return restore_schema18_source(path, raw).decode("utf-8")
