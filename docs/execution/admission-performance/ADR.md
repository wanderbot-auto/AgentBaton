# ADR — batch inspection work, retain independent admission boundaries

Status: implemented cb82ac6, authored377+4/lints and real local comparison passed; independent/Mac verification pending.

Static and actual local profile identify two full10-Git inspections, one per reservation and launch, not a duplicate inside one transaction. Reusing the first as authority after durable writes would weaken drift checks. Keep both, including grant/revocation/expiry/configuration/executable and final launch checks. Optimize only each inspection's readonly queries with the same hardened Git launcher; no cross-request cache or new authority token.

A no-includes NUL-separated config list can audit include/conditional include, partial-clone/promisor and executable-filter declarations at once. Read only first key/value delimiter; multiline values are data. Bounds/errors remain fail closed. Keep Git's sparse boolean query to preserve its actual empty/numeric/duplicate semantics; reaching4 calls is not worth a new approximate parser. Batch fixed rev-parse outputs only when unambiguous; newline paths fall back to original independent queries. Combine index status flags/modes before status, preserving hidden flags and gitlink rejection without inspecting submodule code. All repository code-execution suppression and15-second/4MiB capture bounds remain unchanged.

This is neither atomic filesystem isolation nor proof of macOS speed. Independent negative safety tests and actual fresh macOS CI remain required after Linux comparison and source review. No timing threshold, expiry or expected lifecycle state is widened to get green.
