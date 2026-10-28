# Goal validation graph UI and Native plan

Baseline 3b99f2075b031c1666b9dc4a3466a5b207749983; branch codex/goal-validation-graph-ui. ACK OWNER001/002 and AB-START-001. Partial TSK003/004/007/008/009/010/011/012, VAL001/002/003/006/007, SEC001/002/006/010, UX004/005/006/007/009/010. No full Goal/130-requirement acceptance claim.

Only this author writes new GoalGraphExecution/API/i18n/fixtures/CSS/tests, minimal GoalPlanning/GoalExecution integration, Native approval.rs/lib.rs with tests and these four records plus ADR. Core/schema/lock/CI/global CSS/status/navigation excluded. Public wire contract and schema12 are owned by Core author. No guessed wire fields.

1. Freeze exact versioned graph DTO/proposal snapshots/fixed methods with Core author. Preserve independent old Goal execution kinds and original tests.
2. Keep a single progressive Goal execution region, folded by default. Show typed legacy and graph history, candidates, validations, Owner decisions and true Task acceptance. Unknown/future/effects/stop/revoke remain distinct; positive gates prevent stale/foreign/fork/ledger regression writes.
3. Implement fixed Native graph and Owner-node methods only. Native-owned bounded complete snapshot pages and final confirmation precede private original proposal ID/revision/key finalize. Cancel does not create an Owner rejection receipt. No renderer generic authority.
4. Add meaningful UI/API/Native failure and exact-intent recovery controls. Run Node/type/build independently, then actual Core12 combination and Native compile/test after explicit target lease. Independent review precedes integration.

Native/Main target not leased to this author at start; no Cargo before explicit handoff. No real product model/account/fee/production/signing/recursive agents/remote writes.
