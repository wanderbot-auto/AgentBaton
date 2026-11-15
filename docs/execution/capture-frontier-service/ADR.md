# Same-tick capture service order

Status: implemented locally, source reviewed; exact Mac validation pending.

A measured846 capture spent529ms between the original queue clock and observed join while real file I/O took less than1ms. This does not isolate pure Actor idle or prove another failure cause. Source has a full unrelated frontier sweep ahead of the already persisted capture. Move that sweep after the unchanged prepared-job service within the same Actor call. Nonfatal early returns are localized in the helper so the full sweep still runs; fatal errors propagate before any later dispatch. Preserve original validators, cancellation, slot limits, fresh prepare/mark/finish checks, owned handles and real join. Never restart the clock or subtract scheduler/verification costs from actual usage.

A new error ordering requires a real lifecycle regression: successfully read and marked capture, later corrupt frontier, original fatal cancellation and join before writer unlock. Only unit tests can install a channel after real I/O; no control exists in normal libraries/release. The test also exercises no-job/missing-clock/expired-clock returns and confirms no new attempts, candidates, acceptance or refunded reservation. Generated malformed data never supplies execution authority.

This reversible scheduling change does not guarantee a loaded Mac can complete every4s attempt. If exact tests still fail, preserve measured refusal and continue diagnosis rather than widen budgets or accept unknown results.
