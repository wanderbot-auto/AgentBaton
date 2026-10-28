# Playwright driver lifecycle — finite CI maintenance

Base: `fb7ff7d861872eec18e010f7b64a0ead63740faa`. Branch: `codex/playwright-driver-lifecycle`. Supporting CI reliability work only; no product permissions or acceptance expansion. OWNER001/002 and existing AB-START-001 boundaries continue unchanged.

The e3cc and fb7 successful Linux UI jobs both retain a pending Playwright initialization task and unretrieved TargetClosedError after the final flow. Preserve those warnings and distinguish them from the Graph's owned-resource receipts. Investigate fixed workflow/last-flow source and matching locked Playwright1.62 source before changing behavior.

Root authorized exactly two old and two candidate driver-only subprocesses: metadata accessor only, no browser/Core/network/new dependency. Preserve actual task state and raw output. If they support a minimal fix, change only the parent's path lookup from a sync context to a public async context run by asyncio.run. Keep every child test invocation/order, bound, check name, dependency, artifact and permission unchanged.

Extract and compile the actual candidate parent Python without executing it; compare exact surgical diff, all remaining YAML and subprocess ASTs; run static infrastructure and diff checks. Freeze a source commit plus these records and lossless evidence. Independent review and the next exact CI remain Root-owned follow-up; do not push or disturb the current fb7 CI.
