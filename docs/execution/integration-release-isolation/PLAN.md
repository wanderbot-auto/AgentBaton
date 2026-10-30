# Integration test-control release isolation

Baseline `b452e4a31dd4d1c9cce7e35282a9c2c9dcf0ff39`; isolated branch `codex/integration-release-audit`. OWNER-001/002 and the development gate were read; product implementation is authorized, production release/signing/real Provider use remain separately gated.

Primary requirement: **AB-REL-003 — 生产无测试后门**, whose acceptance requires removing and detecting test features/entry points in release packages. **AB-REL-001 — 测试层级明确** governs these evidence limits. This slice is not mapped to AB-SEC-007 (different requirement).

The existing release audit checks the actual default-feature application executable using strings/grep and fails closed when inspection fails. Its regex already covered11 markers, including Factory's Native test-control kind, but omitted `native_integration_os_dialog_fixture`, defined separately at `apps/desktop/src-tauri/src/native_owner_dialog_test/controller.rs:104`. Add only that exact literal; preserve every existing regex term, check, success/failure message and error exit behavior.

Add a new infrastructure test that runs the real shell script over private non-executable byte fixtures. Each old marker and the new marker is isolated; legitimate production integration names must pass. Controlled fixed mock tools exercise strings failures (after clean and forbidden output), grep error, and missing inspector/input. Assert no PASS on any failure, inspection temporary cleanup and unchanged fixture bytes/identity/mode. Never execute fixture contents or touch a release executable. No new dependencies.

Source-only/canary acceptance does not prove real Mach-O/default-feature build isolation. Independent source review, the broader infrastructure suite and actual Apple Silicon/Intel release audits remain Root work. No Cargo, Node, Actor, CI, signing, upload, target mutation, remote push or production release is authorized by this leaf.
