# Static repair review — b01c3e6

The desktop import failure in e942 is repaired at the source level by explicit direct `raw-window-handle = "=0.6.2"` and direct imports. Locked source `b01c3e683936ffec85882858c77620133b7e0c6c`.

Independent TOML comparison proves the sole lock change is adding the existing raw-window-handle dependency edge to the desktop root package. Every package/version/checksum and other dependency edge remains identical. No crate version, approval permission or OS capability was changed. lib.rs and approval.rs remain whole-file identical to e942. Removing only the import substitutions reproduces the entire native_parent.rs behavior byte-for-byte, including lifecycle tests.

This is a static repair candidate, not a successful compile or Mac runtime verdict. Root is running actual compilation separately; results must be attached to the exact composed source. The original e942 report and real E0433/exit101 remain preserved, with COMPILE_ERRATUM explicitly withdrawing the missed Android-only reexport feasibility claim. Reviewer did not run Cargo, acquire a target or modify product files.
