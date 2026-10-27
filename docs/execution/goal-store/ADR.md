# ADR — Goal authority, persistence and raw evidence

Status: implemented under primary-approved public contract; independent final Backend review pending.

An existing ExecutionGrant is not Goal approval. Separate native kindgoal binds a complete server-resolved snapshot and authorizes only displayed children/depth changes via same-root CAS. Complete old/new history is retained; existing attempts keep their revisions. Other permissions/bindings/ceilings and shared counts stay intact. Private authority is reconstructed only after actual approval/result/event checks, never from Deserialize or Agent labels. Partial validation is not Goal/Task acceptance.

Schema 9 follows safely merged Core 8 and preserves private pre-v9 plus historical pre7/pre8 backup semantics. Grant CAS/history, actual budget reconciliation, scope/result and events share one IMMEDIATE transaction. Ready preview creates no reservation/attempt/dispatch/acceptance. Opaque Goal evidence/action history requires future actual scheduler reconciliation and cannot be cleared by new keys/revisions.

Fixed input hashes are checked from actual bounded bytes after writer/fence guards. Optional validator references require actual native-confirmed versions and exact full contracts. They grant no new action rights: consumed one-shot Command policies reject new use; owner criteria remain separate new rules.

Plan wire is scope_id+original plan_json string. Direct strict typed decoding under 256KiB preserves duplicate-key rejection; exact original text defines replay identity and typed canonical serialization defines revision digest. Historical committed replay does not resurrect revoked permission.

The native 16KiB cap remains unchanged. Full immutable grant fields appear once with explicit before/after ceiling/revision references that reconstruct exact grants. Full approved_contract_json is lossless, not digest-only/truncated. Full private-record digests plus source re-resolution bind metadata. Actual native dialog/manual readability is unrun.

Actual execution budget rows influence prepare_attempt, so preview-only maxima are insufficient. Missing rows inherit recovered consumed/active facts, existing counts move upward only, and established total/parallel ceilings stay exact. Reconciliation events and counts roll back atomically on approval failure. Runtime/output/token/cost history remains unknown when unmeasured.

Existing oversize meter/idempotency rows are never absence or zero-use evidence. One SQL CASE checks octet_length before materializing bounded text, distinguishing missing rows from oversized ones without a second-read race. Regressions cover both; no cap/assertion/timeout/permission was weakened.
