# Quiet Canvas final integration checkpoint

Source frozen at `bf042ec39bb49820cbac5704d5b12707067e040b`, with conditional Git guard merge `5fcfd698` from `32ca76eb`. All 18 older workspace protections and the raw-object identity guards remain. Composite library SHA is `796e8cf7b2f92052f01b4140ac7f76fa47631959190138fbaa755ff7b682491e`.

Actual local checks: 24 workspace tests, 199 top-level Rust tests, 47 UI tests, typecheck/build/fmt/Clippy, and 22 real browser/UnixIPC checks with 8 fresh screenshots. The new UI test uses asynchronous act for initial mocked responses and verifies truly deferred scope responses cannot enable grants; no timeout or old assertion was relaxed. Prior evidence is archived in EVIDENCE.before-include-ee6.json and earlier histories.

Primary must arrange quiet_canvas_review independent review, push the dedicated UI branch/update PR#7, and execute actual exact-head macOS CI. No adoption/profile/schema5, later native-provider choices or lock changes were added. Old F3 CI and prior native failures remain evidence, not proof of this candidate. Full remaining product scope and external model/account/signing/release gates remain unchanged.
