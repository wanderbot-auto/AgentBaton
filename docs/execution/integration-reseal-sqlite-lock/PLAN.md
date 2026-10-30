# SQLite descriptor lifetime regression

Verify the narrow descriptor-lifetime operation used by the corrected integration reseal: an original DatabasePin survives a temporary Arc clone/drop while an independent process attempts BEGIN IMMEDIATE. Use real default-WAL Store and separate rollback DELETE cases. Separately demonstrate the raw same-inode descriptor-close POSIX control under DELETE, without asserting the WAL writer lock resides on the main DB inode.

This is one test-only slice under the existing integration safety requirements and AB-START-001. No new product permission, IPC control, schema, deadline or reservation is introduced. A complete valid IntegrationStageTicket/Proof fixture is not replicated. The existing actual integration IPC chain must separately verify full reseal/admission behavior.
