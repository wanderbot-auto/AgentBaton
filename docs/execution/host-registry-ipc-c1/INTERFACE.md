# C1 implemented contract and limits

Four fixed arms in existing dispatch after owner token, exactAPI1 and request-ID validation. No actor queue/scoped-tool shortcut, generic dispatcher, automatic retry/startup/contact or new authority. Existing Request Value semantics retain absent/null None; original raw duplicate keys/spans are not recoverable from Value. Serialized params are bounded16KiB before strict typed decode, then actual Store validation.

- hosts.register: exact HostRegistration schema1; required stable idempotency_key, expected_revision=None. Returns the original immutable HostCreationReceipt.
- hosts.find_registration: closed{key}, both mutation fields=None. Returns closed{lookup_version:1,key,receipt:<original receipt>|null}. Errors never become null; null is an audited successful absent snapshot, not proof no in-flight commit.
- hosts.get: closed{host_id}; readsNone-only. Existing typed registry UUID view, never expected protocol Host ID lookup.
- hosts.list: existing closed HostListRequest, readsNone-only; existing bounded page/cursor semantics.

Host revision1/created_sequence stay separate from outer global event revision. Existing Store errors/code/side-effect state pass unchanged. Known committed result serialization/bound failure is conservative OUTCOME_UNKNOWN, never not_started; no second create or new key.

Post-guard successful Host responses use a bounded serialization check against the exact current handle envelope, exact request-ID escaping, maximum-u64 revision digits, empty warnings and one final LF. Total<=512KiB; MAX_HOST_PARAMS16KiB remains distinct from global1MiB request/32MiB response limits. No central invalid-envelope error semantics were changed. Current legal DTOs have Store reserve; review clarified framing, not a reproduced liveDTO overflow. Tests now append the LF at exact boundary and refuse+1.

Tests use real SQLite and actual A helper migration in a private Daemon object and call handle directly. Ordinary Store18 stays unchanged, schema18/future20 calls fail without migration/records. A fixture listener is bound, but no actor run/new Host socket/Tauri/remote transport test is claimed. Twelve new cases include two supplementary codec boundaries; four old wire/frame controls remain untouched.
