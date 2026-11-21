"""Restore the exact pre-handshake lib.rs for historical Host source checks."""
from hashlib import sha256

# Full reviewed native-handshake source and the 02f28862 Git blob.
_CURRENT_SHA256 = "732cdee38bf31f4dbcbe5d4a86f5626ea3010f08a4600f2a47e67ad03be9c204"
_PREDECESSOR_SHA256 = "338fbef7689416a71ec101aaad3ac5afb91e433c55d5c1c4bf413242108944d5"

_CURRENT_GUARD = (
    b'        || response["result"]["database_schema_version"].as_u64() != Some(19)\n'
    b'    {\n'
    b'        return Err("Authenticated daemon requires exact database schema19; '
    b'old/future/unknown schema is unsupported".into());\n'
)
_PREDECESSOR_GUARD = _CURRENT_GUARD.replace(b"Some(19)", b"Some(18)").replace(
    b"schema19;", b"schema18;"
)
_CURRENT_TEST_START = (
    b"    #[test]\n"
    b"    fn current_schema_requires_authenticated_exact_typed_nineteen() {\n"
    b'        let good = json!({"result":{"api_version":1,"database_schema_version":19}});\n'
)
_CURRENT_TEST_END = (
    b"    #[test]\n"
    b"    fn generic_owner_path_cannot_finalize_integration() {\n"
)
_PREDECESSOR_TEST = b"""    #[test]
    fn current_schema_requires_authenticated_exact_typed_eighteen() {
        let good = json!({"result":{"api_version":1,"database_schema_version":18}});
        assert!(require_current_database_schema(&good).is_ok());
        for bad in [
            Value::Null,
            json!(17),
            json!(19),
            json!(18.0),
            json!(true),
            json!("18"),
        ] {
            let mut response = good.clone();
            response["result"]["database_schema_version"] = bad;
            assert!(require_current_database_schema(&response).is_err());
        }
        let mut error = good.clone();
        error["error"] = Value::Null;
        assert!(require_current_database_schema(&error).is_err());
        assert!(
            require_current_database_schema(&json!({"result":{"database_schema_version":18}}))
                .is_err()
        );
    }
"""


def restore_host_native_schema18(source: bytes) -> bytes:
    """Accept only the fixed current lib.rs and recover the exact previous bytes."""
    if type(source) is not bytes:
        raise ValueError("Native handshake inverse requires raw bytes")
    if sha256(source).hexdigest() != _CURRENT_SHA256:
        raise ValueError("Unknown complete native schema19 source")
    for anchor in (_CURRENT_GUARD, _CURRENT_TEST_START, _CURRENT_TEST_END):
        if source.count(anchor) != 1:
            raise ValueError("Expected one exact native schema19 anchor")
    start = source.index(_CURRENT_TEST_START)
    end = source.index(_CURRENT_TEST_END)
    if end <= start:
        raise ValueError("Native schema19 test anchors are out of order")
    restored = source[:start] + _PREDECESSOR_TEST + source[end:]
    restored = restored.replace(_CURRENT_GUARD, _PREDECESSOR_GUARD, 1)
    if sha256(restored).hexdigest() != _PREDECESSOR_SHA256:
        raise ValueError("Native handshake inverse did not restore exact schema18 source")
    return restored
