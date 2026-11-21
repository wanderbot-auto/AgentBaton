# Proposed Host wire and pending-intent contract

Proposal only; Core record/receipt canonical format remains fixed B4e/B039. No implemented native/IPC/UI behavior is implied.

## Four Core methods after the existing owner boundary

| Method | Params and envelope | Result |
| --- | --- | --- |
| hosts.register | Exact closed HostRegistration schema1; required stable idempotency_key; expected_revision=None | Original immutable HostCreationReceiptV1 unchanged |
| hosts.find_registration | Closed {key}; both mutation fields=None | Closed {lookup_version:1,key,receipt:HostCreationReceiptV1|null} |
| hosts.get | Closed {host_id}; mutation fields=None | Existing HostRegistrationView {view_version:1,record,registration:registered} |
| hosts.list | Closed HostListRequest {limit,after_created_sequence?}; mutation fields=None | Existing HostRegistrationPage {view_version:1,hosts,next_created_sequence} |

Native/API response envelope stays request_id/result-or-error/global revision/warnings. Never bind a Host expected revision or created_sequence to the envelope global revision. Host metadata input needs no client-supplied registry UUID/status/grant. Only get/list expose already-derived inventory facts; no endpoint contacts a Host or resolves refs. lookup null is a successful bounded snapshot with neither matching receipt nor record after integrity checks; errors/unsupported/corruption/timeouts are never null. Absence cannot exclude an in-flight original commit; retry is the original key/body only. New create capacity never masks historical replay. get NOT_FOUND is distinct from schema unsupported and read integrity failure.

## Persistent intent and seven fixed native commands

Proposed private file host-registration-recovery.json plus host-registration-recovery.lock in the existing anchored Runtime directory. Reuse ClientRecoveryJournal I/O and wrapper version1, preserve managed/task-message journals untouched. At most1 Host intent;16KiB whole journal bound. Closed Host intent fields: intent_version=1, entry_id (native generated UUID), request_key (captured UI UUID), registration (validated frozen metadata), registration_sha256 (canonical typed metadata digest), created_at (server-created UTC marker timestamp). The key/digest/metadata are equality/correlation only, never execution authority. Validate bounds before dispatch. Maximal legal schema1 wrapper fit is an actual future test prerequisite, not an assumed result; inability to persist means no Core create call. No TTL/abandon/remove command is introduced.

| Native command | Fixed behavior |
| --- | --- |
| hosts_register {registration,idempotencyKey} | Validate via Core-owned Host intent API, durably reserve original key/body, then call only hosts.register. A pending entry prevents a fresh key. Return closed {submission_version:1,entry_id,receipt} inside the normal envelope on confirmed success; keep journal until explicit matching acknowledgement. |
| hosts_list {request} | Direct fixed hosts.list read, call(...,false); no startup/probe/contact. |
| hosts_get {hostId} | Direct fixed hosts.get read. |
| hosts_registration_pending {} | Private local read returning closed {pending_version:1,intent:null|HostIntent}; usable with disconnected daemon. No Core create/auto retry. |
| hosts_registration_reconcile {entryId} | Re-read exact current marker; Core hosts.find_registration with journal key only; validate found receipt against original metadata. Return closed {recovery_version:1,entry_id,lookup}. Do not clear journal or retry when absent/error. |
| hosts_registration_retry {entryId} | Explicit owner UI action only; capture current marker, successfully reconcile original key first. Found returns original receipt, absent submits exact journal metadata/key. Read failure does not trigger a write. No auto new key/recipient/identity/config change. |
| hosts_registration_acknowledge {entryId,hostId,createdSequence} | After UI has parsed/correlated a displayed confirmed receipt, re-read marker and fresh Core lookup, validate receipt/metadata/id/sequence and exact current entry, then clear only that local intent. No Core row/event/receipt deletion. Reply loss here is not another Host create. |

Keeping the journal until acknowledgement is necessary across the second response-loss boundary: if Core commits, native receives a receipt and then Tauri/WebView reply is lost, clearing the intent before returning would allow a fresh key and duplicate registration. Pending/reconcile/retry must still find that original operation after UI/native reopen. Acknowledgement is local bookkeeping after confirmed result, not release approval or new authority. It never writes a tombstone/approval/connection or redefines Core's immutable creation receipt. Native may return the known receipt with a warning when local acknowledgement fails, while retaining pending intent and blocking a new key. Do not treat successful Core receipt plus failed cleanup as a failed business create.

Reuse Core-derived typed intent/receipt DTOs through Core's existing dependency, not a new native remote-host or serde manifest dependency. The only proposed unfreeze in existing B host_registry.rs is crate-visible existing receipt validation so the Core intent child can check semantics/correlation; no new parser or duplicate Host DB. Native Value is only the Tauri JSON boundary: serialized byte limits and Core's raw decoder are distinct from original WebView argument bytes. No generic method/path/argv parameter is exposed.

## Error/recovery policy

| Observation | UI state and allowed next step |
| --- | --- |
| Native validation/journal failure before dispatch | Show precise local failure, refresh private pending journal; no new Core write on persistence failure. If local fsync/rename outcome is unknown, retain/re-read it rather than assuming absent. |
| Auth/API/schema mismatch | Unsupported/local-auth error, keep pending, do not spawn a second daemon or silently downgrade/open another DB. A coherent explicit reconnect uses existing startup policy outside Host refresh. |
| PROVIDER_DISCONNECTED/OUTCOME_UNKNOWN, rejected Tauri promise, malformed result | Outcome unknown; keep captured key/body and private intent, explicit read-only reconcile. No false offline-Host/confirmed-stop labels. |
| Lookup receipt found and exactly matches capture | Show original registry UUID/revision1/actual SQL sequence and declared metadata; acknowledge exact entry after parsing/display. |
| Successful lookup null | Show unresolved original request, retain capture; explicit original-key retry allowed, no new-key retry/forget. A prior in-flight commit is still possible. |
| CONFIG_CONFLICT or receipt/key/metadata mismatch | Preserve record, show conflict, do not adopt another operation's Host or clear/rekey. No automatic repair/abandon exists. |
| RESOURCE_LIMIT at new create | Show capacity issue; retain original key, because later exact replay must still reconcile. No silent deletion or budget/state reset. |
| Confirmed Core result, private clear/ack fails | Registered receipt remains known; show local recovery warning, keep entry and block fresh creation until exact acknowledgement succeeds. |

There is a deliberate unresolved lifecycle boundary: a permanently conflicting/invalid pending capture cannot be forgotten through this cut; any future abandon needs a reviewed seal/fence against a late original commit, not a local file deletion. Do not add such authority to make the form convenient. This proposal keeps creation/recovery meaningful with zero real remote contact, Provider, spending, permission expansion or production release.

## Settings rendering

One advanced Hosts disclosure in existing Settings. Prop name daemonConnected means local daemon only. Form supports every declared schema1 field and mappings0..8 without browsing/copying private keys/global configs. Status labels: registered; syntax validated at creation; identity/config/key references unresolved; remote not contacted/probed; capabilities unknown; execution disabled. Declared macOS/Linux is not tested platform support. expected protocol Host ID and generated registry UUID have separate labels. known_hosts FILE SHA-256 never masquerades as an SSH server public-key fingerprint.

Refresh lists local registry and pending intent; it is not a heartbeat. Preserve stale known data with an explicit local refresh error, rather than replacing it by empty/offline. Future unknown versions fail visible rather than fabricated defaults. On unmount/close, cancel only rendering/subscriptions, not the Core transaction, intent or background daemon. On reopen, read pending first and reconcile; no blind re-submit. JS parsers use closed objects, TextEncoder UTF-8 limits and safe integers; they cannot claim original JSON1.0 lexical rejection after browser parsing. No new navigation, terminal, command panel, connection/probe selector or task routing is introduced.
