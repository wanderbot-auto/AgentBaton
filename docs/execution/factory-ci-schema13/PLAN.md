# Factory schema13 CI wiring

Implement the dependency-ready CI checks for AB-COR-002 and AB-TSK-002 through AB-TSK-008 from Factory baseline `8c6ea33c493d1936dfe7521d1bb96f0e410319ad`. This is test wiring, not acceptance of those requirements or the full 130-requirement design.

Preserve the original 12 UI flows and all check names, runners, permissions, action pins, dependencies and 25/20/45-minute budgets. Build both actual tools, bind their raw SHA-256 values plus checkout/tree in the candidate, require exact integer schema13 in preflight and SQLite checks, and append the existing Factory normal/cancel/unknown driver modes. Integrate the reviewed upstream driver byte-for-byte without authoring changes to it.

Verify only finite Python/YAML/AST/generated-SQLite controls within this task. Preserve historical failures and arrange independent review before Root integrates. No Cargo, Core, browser, native execution, remote operations or CI trigger is part of this leaf.
