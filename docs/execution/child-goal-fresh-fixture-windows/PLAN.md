# Fresh positive browser fixture windows

Requirements TSK009/TSK011, cancellation/timeout/permission boundaries; AB-START-001. Root baseline `20c568dc575189b5c07bb18dd7e7f39f66982534`; isolated branch `codex/child-goal-fresh-fixture-windows`. Allowed changes only existing child Goal harness and these four records. No production UI/Core/timer/DDL/lock/workflow change and no actual browser/Cargo/npm/Actor run.

## ADR: separate initial wall permission from effective execution budget

Original lost-start correctly reconciled the one deliberately lost reply, but later parent dispatch was only5ms before the original30s Grant expiry. Original cancel Work hit its4s deadline before the UI could request cancellation; outcome_unknown plus cancel_requested is correct and is not Cancelled. These original failed fixtures/bytes remain unchanged and are not relabelled as passed.

For a NEW fixture only, declare Native Grant max_duration_ms60000 and Factory Scope TTL60000 before their first approval, including the actual initial UI form. Keep all six root/project budget bindings at original30000 and all other ceilings unchanged. No renewal, reapproval of expired authority or runtime mutation.

Normal/lost-start still declare child5000, Work4000 and Owner1000. Only cancel initially declares child12000 and Work10000, with original Owner1000 and root/project30000. The extra initial cancellation headroom permits a real before-timeout UI stop; it never makes an unknown result count as Cancelled. Retain raw saved-plan/digest/Owner-order checks and all existing cancellation success/failure assertions.

Pure/AST checks verify all three declarations, fresh wall/effective distinction, existing timeout/permission assertions and default NOT_RUN exit2. Root owns independent review and the only actual rerun lease; this source does not guarantee runtime success or timing.
