# PR #33 macOS release source check

Start at reserved base 70bcc2aeada84e454a128a4ef38db7251cc9b1fd. The designated integration candidate is 3e8171314b78fb2f35a7001f268a594e1f962b02. Compare exact relevant build inputs before replaying its packaging command. Preserve the exact clean source check and both receipt checks.

Run the pinned Tauri CLI with a Cargo runner probe to capture tracked changes before compilation. Isolate any remaining changes by release command. Test a minimal correction with pinned inputs and a real positive and negative source check. Keep raw logs and report what can be executed within available Apple Silicon disk space.
