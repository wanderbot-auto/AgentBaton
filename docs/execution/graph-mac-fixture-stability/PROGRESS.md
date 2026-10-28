# Graph Mac fixture stability — progress

Source frozen at c05f8cee4966b8c76b6e8de7181d5b9a21572dfe on codex/graph-mac-fixture-stability, baseline652. The sole source file SHA is227d3fcb499969d81ae3777ff3f72441b0aea7096f8452fa15492ce22c0b245b.

The prior independent source-only investigation is frozen separately (review7f97a1ba705d5eb711808fb40d828cbe442f77ec3f50a33e88554d7a8c7cd3b9, manifest10005c0cec3ff907d276a32ce2074282deceafef14d4a21a943e887a742e1a79). Root then explicitly reassigned this agent as fix author; this author cannot provide independent approval of the new fix.

Implemented exactly the approved fixture changes: the enclosing Integrity Grant10000→30000, diagnostics on failed response/transport including method/key/call elapsed, named Grant phases with actual approved/expiry/current-clock values, short recovery name restore, same-length private bind probe with error kind/errno. The probe runs after actual snapshot creation and original daemon shutdown, so it does not delay taking the in-flight crash image. It binds only in a newly created private random sibling directory and removes only its successfully bound private socket and that fresh directory.

Actual standalone rustfmt parse/check and git diff --check pass. A mechanical inverse restores the whole baseline file byte-for-byte after removing only the explicitly approved changes; all55 test names and all other bytes/assertions/control values are preserved. Cargo/typecheck/actual55 tests/Clippy/probe runtime/Mac CI have NOT_RUN: no target lease yet.

Retained tooling failures: first bare rustfmt command was absent from PATH (tool output, no redirected raw shell log); the installed standalone absolute binary was then used. The first mechanical verifier replaced two identical Grant JSON lines in its inverse mapping, causing a checker-only failure. Its script/log/residual diff are preserved; only that verifier was limited to the intended first occurrence. Product/source was not edited to satisfy that check.
