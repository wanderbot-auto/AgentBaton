"""Reverse the reviewed Host dispatcher additions for the historical source check."""
from hashlib import sha256


# Whole lib.rs at the reviewed combination 8aa4483d, and its 1f756ce2 parent.
_CANDIDATE_SHA256 = "338fbef7689416a71ec101aaad3ac5afb91e433c55d5c1c4bf413242108944d5"
_PREDECESSOR_SHA256 = "6650edec5c8b8ac3fcdf5ff03813eafe40b5847609fc008cedbf185c96102cdb"

_MODULE_SITE = (
    b"mod client_recovery_bridge;\nmod detached_daemon;\n",
    b"mod hosts;\n",
    b"mod integration_approval;\n",
)
_HANDLER_SITE = (
    b"        data_workspaces_get,\n        data_workspaces_cancel,\n",
    b"        hosts::hosts_register,\n"
    b"        hosts::hosts_list,\n"
    b"        hosts::hosts_get,\n"
    b"        hosts::hosts_registration_pending,\n"
    b"        hosts::hosts_registration_reconcile,\n"
    b"        hosts::hosts_registration_retry,\n"
    b"        hosts::hosts_registration_acknowledge,\n",
    b"        connections_list,\n        connections_register,\n",
)


def restore_host_native_bridge_v1(source: bytes) -> bytes:
    """Accept only the pinned composition, restoring the exact predecessor bytes."""
    if type(source) is not bytes:
        raise ValueError("Host bridge inverse requires raw bytes")
    if sha256(source).hexdigest() != _CANDIDATE_SHA256:
        raise ValueError("Unknown complete Host bridge source")

    restored = source
    for before, insertion, after in (_MODULE_SITE, _HANDLER_SITE):
        anchor = before + insertion + after
        if restored.count(anchor) != 1:
            raise ValueError("Expected one exact Host bridge anchor")
        restored = restored.replace(anchor, before + after, 1)
    if sha256(restored).hexdigest() != _PREDECESSOR_SHA256:
        raise ValueError("Host bridge inverse did not restore the exact predecessor")
    return restored
