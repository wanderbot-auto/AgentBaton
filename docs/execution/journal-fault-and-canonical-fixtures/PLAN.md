# Journal fault and canonical IPC test fixtures

Base b4dc2dcc549b11d2b9448147b5ae0dc3cf96174f; requirements AB-SEC-007, AB-START-001. Preserve the full real command/journal/restart contract and real owned Data I/O busy/read-only contract.

1. While the injected reject_terminal_command trigger remains, prove exact cold-open INPUT_CHANGED/not_started and unchanged run rows, events, fences, schema and marker.
2. Remove only that generated fault trigger, prove the schema delta is exactly its removal, then execute the original reopen/same-key outcome_unknown/no-reexecution contract with the fence still held.
3. Create only the affected owned-Data IPC fixture below canonical /tmp with a short generated prefix and0700; retain production symlink refusal and all original I/O/authority/cleanup checks.
4. Source/format checks here; independent review and complete actual command14/Core229 execution by Root before acceptance and exact-candidate Mac CI.
