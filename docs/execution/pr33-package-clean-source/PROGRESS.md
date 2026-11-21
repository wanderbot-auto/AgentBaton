# Progress

Offline `npm ci` installed the task-local pinned Tauri CLI 2.12.4. Desktop Cargo metadata with the committed manifest and lock passed under `--locked --offline`. Available local disk was 1.2 GiB before npm installation.

On Apple Silicon, the pinned Tauri frontend built and entered its Cargo runner with `build --bins --features tauri/custom-protocol --release`. The tracked checkout was clean at runner entry. The probe intentionally exited before compilation.

Running real unlocked Cargo metadata with `tauri/custom-protocol` then rewrote the committed desktop lock at lines 79–86, deleting one extra blank line. The TOML graph of 443 package records remained identical. The before and after SHA-256 values matched the earlier actual Tauri build recorded in the repository's PR33_LOCAL_PACKAGE_BUILD.json. That record's complete locked replay kept the original lock bytes and passed its then-current App build and DMG packaging.

The affected inputs are byte-identical in the observed source and the designated integration baseline. The task checkout advanced to the latter. The correction forwards `--locked` to Cargo from the actual Package job. The historical workflow inverses retain their exact input hashes, while the new source guard tests run the current anchor against a private Git checkout before and after a tracked mutation. Focused tests passed 37/37.

The source change was frozen at `9176c4fcdf87ca55a8992271892d5145bdbb79d5`, with designated parent `3e8171314b78fb2f35a7001f268a594e1f962b02`. Two bounded native controls then used Tauri 2.12.4 and Cargo 1.91.1. The original invocation forwarded `build --bins --features tauri/custom-protocol --release` and real Cargo resolution rewrote only the lock's redundant blank line. The fixed invocation forwarded `build --locked --bins --features tauri/custom-protocol --release` and left tracked bytes clean. Resolved ARM64 metadata was byte-identical. Both probes stopped intentionally before compilation. The exact hosted source guard had passed at checkout and failed only after packaging. The hosted runner did not print which clause failed, so the lock cause is established by local reproduction and earlier full-build evidence.

The initial infrastructure run used system Python and failed one test because jsonschema was missing. The original log is retained. The rerun with the project-pinned jsonschema 4.26.0 and PyYAML 6.0.3 validated 20 action references and ran 1,151 tests, with 1,135 passed and 16 original skips. The optional download of the hosted Cargo version did not complete. It did not affect the local native control.

The four source files fall within the reservation relative to the designated integration parent. The standard scope script still compares against the old upstream ref and flags ten files already present in the designated parent. Exact candidate-relative inspection confirmed every listed path is inherited. The merge preview against the designated parent passed.
