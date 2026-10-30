# Progress

Source frozen at `ea9bcd12fc78016922c86b40ed84108dbf918b70`: product.yml adds exactly three Mac-matrix steps (103 lines), and the new test file adds 17 pure controls (319 lines). No old workflow byte is changed: removing the contiguous three-step addition exactly restores the baseline. Removing those parsed steps also restores the entire baseline YAML document, preserving every existing job, action, permission, step and command.

The missing feature-enabled --lib test configuration is explicit, debug, locked and serial. Existing default Native tests and feature --bin test/build remain byte-identical. Some shared library tests will execute in both configurations; this is not an additive unique-test count.

The driver wrapper executes both last-page and first-page configurations, retaining actual exit codes or null on launch failure. A first failure still permits the second case to execute, while the whole step remains failed. Raw output stays private. The always projector independently requires driver exit0 and successful strict projection for each case. It does not turn partial/missing output into a pass. Native report groups carry the existing current17 counts and ordered Owner/policy event contract; the finite collector explicitly does not establish visual review or independent driver cleanup.

Final pure run: 17 tests PASS, 0.990s. The initial WIP run also passed17/0.917s, followed only by removal of an unused test import. Two inline programs compiled as Python without executing their mains; new test AST/YAML/diff checks passed. No failing author test or product attempt was hidden. No actual product or Mac execution occurred.

Known baseline boundary: release audit command is preserved, with its existing Owner/Factory/preview/copy markers. Its regex does not separately name `native_integration_os_dialog_fixture`; this was reported to Root for a separately authorized scanner writer if desired. This slice adds no Rust/test-control marker and does not modify release isolation.
