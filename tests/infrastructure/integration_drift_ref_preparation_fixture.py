"""One exact source-only drift fixture layer; no Git, SQL, Rust or daemon run."""
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
PATH = 'crates/core/tests/fixtures/integration_runtime.rs'
FIXTURE = ROOT / 'tests/infrastructure/fixtures/integration_drift_ref_preparation_inverse.json'
FIXTURE_SHA256 = '6a13ca55bdd00de07403382fc917082940d43710c565b1a9b5f6547be505a51a'
INPUT_SHA256 = 'a7bc171e788081055010bcb01ec074bb2a791f4b3c4d1121c60880c3113dc872'
CURRENT_SCHEMA18_SHA256 = 'bf9a348a6923432c0ecb66d5fefbe1b40aee9d46729eddddcd62e04079d3d504'
RESTORED_SHA256 = '4fa5b264825980fb2cfe150ed94607854541c71577766587072a202ea60ad30e'
BASE = 'd3098c40921ec3f3cb46266e60234775b156f13b'


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def load_inverse(raw):
    if type(raw) is not bytes or len(raw) > 131072 or sha(raw) != FIXTURE_SHA256:
        raise AssertionError('Unknown exact drift source inverse')
    value = json.loads(raw)
    if (type(value) is not dict
            or set(value) != {'contract', 'base_commit', 'path', 'input_sha256', 'restored_sha256', 'edits'}
            or value['contract'] != 'agentbaton.integration-drift-preparation-exact-source-inverse.v1'
            or value['base_commit'] != BASE or value['path'] != PATH
            or value['input_sha256'] != INPUT_SHA256
            or value['restored_sha256'] != RESTORED_SHA256
            or type(value['edits']) is not list or len(value['edits']) != 7):
        raise AssertionError('Unknown drift source inverse scope')
    for edit in value['edits']:
        if (type(edit) is not list or len(edit) != 2
                or any(type(part) is not str or len(part) > 32768 for part in edit)
                or not edit[0]):
            raise AssertionError('Unknown exact drift inverse edit')
    return value['edits']


EDITS = load_inverse(FIXTURE.read_bytes())


def restore_before_drift_ref_preparation(text):
    if type(text) is not str or len(text.encode()) > 1048576:
        raise AssertionError('Exact drift inverse expects bounded source text')
    # Reverse only the current-host schema checks and Darwin UID decoding.
    # The original digest still checks the complete fixture before its inverse.
    assertions = (
        ('["database_schema_version"],\n            18', '["database_schema_version"],\n            17'),
        ('    assert_eq!(version, 18);', '    assert_eq!(version, 17);'),
        ('''    let mut number = || words.next().unwrap().parse::<u32>().unwrap();
    let pid = number();
    let parent = number();
    let group = number();
    // Darwin ps prints some unsigned uid_t values using signed notation.
    let raw_uid = words.next().unwrap();
    let uid = if raw_uid.starts_with('-') {
        raw_uid.parse::<i32>().unwrap() as u32
    } else {
        raw_uid.parse::<u32>().unwrap()
    };
    let identity = ObservedCommandIdentity {
        pid,
        parent,
        group,
        uid,
    };
''',
         '''    let mut number = || words.next().unwrap().parse::<u32>().unwrap();
    let identity = ObservedCommandIdentity {
        pid: number(),
        parent: number(),
        group: number(),
        uid: number(),
    };
'''),
    )
    if sha(text.encode()) == CURRENT_SCHEMA18_SHA256:
        for current, original in assertions:
            if text.count(current) != 1:
                raise AssertionError('Exact schema18 fixture assertion required')
            text = text.replace(current, original, 1)
    digest = sha(text.encode())
    if digest == RESTORED_SHA256:
        return text
    if digest != INPUT_SHA256:
        raise AssertionError('Unknown drift fixture source')
    for expected, restored in EDITS:
        if text.count(expected) != 1:
            raise AssertionError('Missing or duplicated exact drift boundary')
        text = text.replace(expected, restored, 1)
    if sha(text.encode()) != RESTORED_SHA256:
        raise AssertionError('Whole original drift fixture not restored')
    return text
