"""Raw workflow splice demonstration. It does not emit a runnable workflow."""
import hashlib
from pathlib import Path

from tests.infrastructure.test_schema17_migration_workflow import (
    FRESH18_WORKFLOW_SHA256,
    restore_workflow_before_fresh18,
    verify_only_six_historical_target_changes,
)


def digest(blob):
    return hashlib.sha256(blob).hexdigest()


def split_reference(workflow):
    if digest(workflow) != FRESH18_WORKFLOW_SHA256:
        raise ValueError("unreviewed workflow")
    desktop_at = workflow.index(b"\n  desktop:\n")
    cut = workflow.index(b"      - *fresh_schema18_gate\n", desktop_at)
    release_at = workflow.index(b"          cargo build --release --locked --bin batond --bin baton\n", desktop_at, cut)
    first_migration = b"      - name: Migrate genuine native schema14 history with corruption and rollback controls\n"
    release_end = workflow.index(first_migration, release_at, cut)
    release_bundle = workflow[release_at:release_end]
    expected = (
        b"          cargo build --release --locked --bin batond --bin baton\n"
        b"          bash scripts/prepare-desktop-sidecars.sh\n"
        b"          cargo test --locked --manifest-path apps/desktop/src-tauri/Cargo.toml\n"
    )
    if release_bundle != expected:
        raise ValueError("unreviewed release and default host group")
    return {
        "unchanged_and_desktop_prefix": workflow[:release_at],
        "release_and_default_host_group": release_bundle,
        "trimmed_desktop_tail_before_fresh18": workflow[release_end:cut],
        "fresh18_native_and_package_tail": workflow[cut:],
    }


def restore_reference(presented, ordered_pins):
    if presented.keys() != ordered_pins.keys():
        raise ValueError("unexpected splice pieces")
    if any(digest(presented[key]) != expected for key, expected in ordered_pins.items()):
        raise ValueError("unreviewed splice bytes or role")
    restored = b"".join(presented[key] for key in ordered_pins)
    if digest(restored) != FRESH18_WORKFLOW_SHA256:
        raise ValueError("workflow reconstruction mismatch")
    return restored


if __name__ == "__main__":
    reference = Path(".github/workflows/product.yml").read_bytes()
    pieces = split_reference(reference)
    pins = {role: digest(blob) for role, blob in pieces.items()}
    required_order = list(pieces)
    reordered = {key: pieces[key] for key in (required_order[0], required_order[2], required_order[3], required_order[1])}
    restored = restore_reference(reordered, pins)
    assert restored == reference
    restore_workflow_before_fresh18(restored.decode())
    verify_only_six_historical_target_changes(restored.decode())
    print("pass exact split reference, reconstruction and existing historical inverses")
    for role, pin in pins.items():
        print(f"{pin}  {role}")

    prefix, release, history, tail = required_order
    invalid = [
        ("relaxed-permission", {**reordered, prefix: pieces[prefix].replace(b"contents: read", b"contents: write", 1)}),
        ("missing-default-test", {**reordered, release: pieces[release].split(b"          cargo test")[0]}),
        ("changed-locked-build", {**reordered, release: pieces[release].replace(b"--locked ", b"")}),
        ("missing-fresh18", {**reordered, tail: pieces[tail].split(b"\n", 1)[1]}),
        ("missing-history", {key: blob for key, blob in reordered.items() if key != history}),
        ("duplicated-native-tail", {**reordered, tail: pieces[tail] * 2}),
        ("wrong-region", {**reordered, history: pieces[tail], tail: pieces[history]}),
    ]
    for name, candidate in invalid:
        try:
            restore_reference(candidate, pins)
        except ValueError:
            print(f"refused inverse/{name}")
        else:
            raise AssertionError(f"inverse accepted {name}")
    print(f"1 source inverse positive, {len(invalid)} negative controls")
