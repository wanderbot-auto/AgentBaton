# ADR: keep debug helper modules outside binary discovery

Status: implemented candidate, pending independent review and native build validation.

Tauri CLI2.12.4 scans every `src/bin` child as a possible bundle binary, including directories that Cargo does not discover as targets. The debug smoke helper directory therefore becomes a phantom default bundle entry. Its entry binary is correctly excluded by `required-features`, so building that extra name would be inappropriate.

Place the helper modules at `src/native_owner_dialog_test/` and point the existing debug/feature-gated smoke entry there. Preserve both files byte-for-byte. This keeps the production module tree, feature requirements, release `compile_error!`, release audit and bundle configuration unchanged. It requires no dependency or CLI upgrade. Future test-only helper modules should likewise live outside `src/bin`.

Evidence: official pinned scanner source plus actual installed CLI controls on generated inert fixtures. This decision removes the discovered phantom entry; real macOS bundling and remaining CI gates still require separate execution.
