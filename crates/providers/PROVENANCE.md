# Installation provenance and execution bindings

`installation::executable_digest` inspects an explicitly selected canonical absolute
regular file and computes SHA-256 with a 512 MiB input bound. It rejects aliases,
invalid digests and changes detected while hashing. `verify_executable` requires
an exact expected digest. `transport::ApprovedInstallation` now requires that pin,
and verifies it immediately before spawning. A changed pin is a local error with
`side_effect_state=none`; it never silently probes or launches a replacement.

The pin detects drift. It does not authenticate a vendor release, verify dependent
libraries/interpreters/argv scripts, or grant permission. An application approval
store must resolve the approval reference and validate the binary's provenance.
Path-based spawning has a check-to-execution race; this code does not claim to
eliminate TOCTOU. It is not an OS filesystem/network sandbox.

`installation::probe_version` executes only the approved canonical executable with
the exact argument `--version`, from an explicit working directory and cleared
parent environment. Any runtime/proxy/CA selectors needed by the program must be
included explicitly through the approved environment. It never reads login files,
copies auth directories, discovers arbitrary executables, or adds API keys. This
command executes code; the named approval must cover the probe. Native programs
may read their own global config despite environment isolation.

The probe has bounded stdout, count-only stderr, nonblocking I/O and a process
runtime deadline. It records UTC epoch milliseconds, the exact binary pin,
observed printable version response, exit status and distinct timeout/output/error
outcomes. It reaps the owned process after signalling its group while its PID is
still retained (`waitid(WNOWAIT)`). This does not control descendants that escape
the group. Hashing is separately byte-bounded; it is outside the process deadline.
A version response proves only what that executable printed. It proves neither
vendor identity, authentication nor any Provider feature.

`connections` models installation source, configuration ownership, identity handle,
host and named connection independently. Managed binaries may use explicitly
connected native configuration; external binaries may use Baton-managed config.
Native config references offer no write-back or auth-copy option. All native
identity fields are opaque store references, not resolved credentials. The module
performs no global configuration writes or credential operations.

`NamedConnection::bind_execution` checks exact references, Provider/route, matching
host, a successful version probe for the pinned bytes, and an evidence manifest.
It returns a cloned immutable input/connection/config/identity/host snapshot.
Account fallback has only the `deny` variant. Required live capabilities cannot be
satisfied by a version probe, documentation or a fixture test. Per-feature status,
source, exact observed version, evidence reference and UTC timestamp remain
separate; there is no global fabricated `supported=true`. The trusted evidence
store must validate provenance of these references; untrusted deserialized records
are never authority by themselves.

## Documented Provider routes

Checked source URLs and downloaded content hashes are recorded in
`documentation-evidence.json`. GitHub sources are bound to immutable commits;
Qoder web documentation is a point-in-time observation.

| Provider | Documented route | Version probe fact | Current runtime evidence |
| --- | --- | --- | --- |
| Codex | `codex-acp` stdio adapter starts the native Codex App Server. Native App Server is a separate protocol route (its pinned README uses `thread/start`/`turn/start`, rather than ACP `session/new`/`session/prompt`). | Adapter README documents `codex-acp --version`. | Live adapter, underlying Codex version and native App Server unrun. |
| Qoder | Official CLI docs specify `qoder --acp` with stdio. | Official CLI reference specifies `--version`. | Live binary, config precedence and authentication unrun. |
| Claude Code | `claude-agent-acp` is a separate adapter around Claude Agent SDK. It is not `claude --acp`. | Pinned adapter `src/index.ts` handles `--version` before SDK startup. | Live adapter, SDK and native CLI unrun. |
| Custom ACP | User-approved executable/argv plus negotiated ACP contract. No default guessed argv. | An explicit installation must support the selected probe; failure stays failure. | No vendor-wide support promise. |

The checked sources contain experimental extensions and alternative auth flows.
These are recorded as documentation, not automatically adopted capabilities or
permission to call models. No automatic `npx`, preview installation, browser login,
API fallback or billing switch is implemented by this slice.

The session-supervision worker in Core is responsible for converting persisted,
validated connection/workspace/grant records into `ApprovedInstallation`. The
descriptor has no deserialization path for renderer authority. Raw contract strings
or a nonempty approval reference do not establish a grant. No second generic
authorization wrapper or duplicate registry is introduced by this transport slice.

Current test dimensions: local contract fixtures, actual selected fixture
executables for version probes, and actual ACP fixture subprocess I/O. Vendor
model calls, real authentication, native macOS verification, persistent connection
registry/daemon/UI integration and managed installation/config materialization
remain separate work.
