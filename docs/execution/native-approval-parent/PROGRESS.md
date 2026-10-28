# Progress

Implemented eleven fixed command paths using framework-injected `WebviewWindow`: connection, execution grant, three validation policies, Goal scope, data workspace copy, legacy Goal execution, graph execution and exact Owner accept/reject. Six routes use the common helper; five use their existing specialized snapshot validators. Renderer selection and all private Core frames are unchanged.

Added checked parent acquisition, a single shared-builder close/destroy hook, per-window active-review exclusion, per-page caller checks, real async plugin dialogs created on the main thread, final dispatch checks, and ten pure lifecycle/exclusion/error unit test bodies. No production unsafe code or dependency/lock change. A review-start deadline fails closed without timing out human review. After the private dispatch gate, a normal window close waits for IPC completion; force-destroy/app-exit does not erase possible Core effects.

Actual checks so far: direct rustfmt parse/check exit 0; git diff --check exit 0; Python source preservation/wiring assertions exit 0. The entire original lib test module and approval.rs are byte-identical to baseline. These are source checks, not Rust test execution. No Cargo/target writer or macOS/dialog/real Core authority probe has run for this revision.

Source review after initial commit found that lifecycle returns to idle when the real callback completes; using idle alone for the startup deadline could reject a long human review. Corrected with a monotonic per-page started flag and a targeted unit body. This was a source finding, not an executed Rust test failure. No human-review timeout is imposed.
