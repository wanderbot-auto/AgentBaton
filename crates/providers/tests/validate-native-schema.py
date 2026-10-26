#!/usr/bin/env python3
"""Validate captured generated-child wire against the exact official Codex schema.

Developer evidence check: uses the repository's existing Python jsonschema dependency.
No vendor executable, account operation, schema download or network resolution occurs.
Pass the explicitly downloaded pinned schema and a native_codex roundtrip capture.
"""
import hashlib
import json
import sys
from pathlib import Path

import jsonschema

SOURCE_SHA256 = "f50e158c34ac55c5cc67c5493df539f1c07b603bd5dd2100a7393eefb62a3e20"
PARAMS = {
    "initialize": "#/definitions/InitializeParams",
    "thread/start": "#/definitions/v2/ThreadStartParams",
    "turn/start": "#/definitions/v2/TurnStartParams",
}
RESULTS = {
    "initialize": "#/definitions/InitializeResponse",
    "thread/start": "#/definitions/v2/ThreadStartResponse",
    "turn/start": "#/definitions/v2/TurnStartResponse",
}
NOTIFICATIONS = {
    "item/agentMessage/delta": "#/definitions/v2/AgentMessageDeltaNotification",
    "turn/completed": "#/definitions/v2/TurnCompletedNotification",
}


def main():
    schema_bytes = Path(sys.argv[1]).read_bytes()
    if hashlib.sha256(schema_bytes).hexdigest() != SOURCE_SHA256:
        raise ValueError("Official schema hash differs from the inspected immutable revision")
    schema = json.loads(schema_bytes)
    # All schema refs must resolve within the hash-verified document, never over network.
    def refs(value):
        if isinstance(value, dict):
            if "$ref" in value and not value["$ref"].startswith("#/"):
                raise ValueError("External schema reference refused")
            for child in value.values():
                refs(child)
        elif isinstance(value, list):
            for child in value:
                refs(child)
    refs(schema)
    pending = {}
    checked = 0
    for line in Path(sys.argv[2]).read_text().splitlines():
        record = json.loads(line)
        frame = record["frame"]
        if record["direction"] == "client":
            method = frame["method"]
            if method == "initialized":
                assert frame == {"method": "initialized"}
                continue
            pointer, value = PARAMS[method], frame["params"]
            pending[frame["id"]] = method
        elif "method" in frame:
            pointer, value = NOTIFICATIONS[frame["method"]], frame["params"]
        else:
            pointer = RESULTS[pending.pop(frame["id"]) ]
            value = frame["result"]
        validator = jsonschema.Draft7Validator({"$ref": pointer, "definitions": schema["definitions"]})
        validator.validate(value)
        checked += 1
    assert not pending
    print(json.dumps({"validated_messages": checked, "official_schema_sha256": SOURCE_SHA256,
                      "scope": "captured generated-child native contract; not live compatibility"}))


if __name__ == "__main__":
    main()
