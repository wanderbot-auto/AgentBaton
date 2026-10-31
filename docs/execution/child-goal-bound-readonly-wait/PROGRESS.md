# Progress

Source 6b2e399f4e235484aa58970413295d060bc786ce, recorded 2026-10-30T23:35:11.651383+00:00. Added bound query and exact typed busy classifier plus one original-window observer; replaced the five existing get polls only. The original generic wait and entire PrivateHost file are byte-identical; a deterministic inverse restores the entire original driver byte for byte, preserving TaskAccepted/receipt/same-run/parent resume/fees/stop/cleanup assertions.

Actual new15 pure controls passed. Combined54 (new15 + original finite13/publisher13/argv13) passed after the unchanged workflow was included in this sparse checkout. First combined54 produced1 failure/2 errors from missing .github/workflows/product.yml; its original log is retained, no assertions were removed. No Cargo, Actor, browser, provider/model, macOS or CI execution by this author.

Independent actual CI diagnosis: PR18 f55 Product41143055493/UI124361646570, synthetic checkout3c09965161af7944376b50c0dec9b9ca1b983c18. Normal onlyCG01/CG02 passed, cleanup_complete=false; one factory_child_goals.get WORKSPACE_BUSY/true/not_started, graph_provider_argv=null, trace1376→1053→1376. Correct stage is after Owner click while waiting TaskAccepted; before-Owner read is1362. Initial diagnosis message misread this and was corrected against exact f55 source. Cancel and lost-start passed separately in that round.

Original failure is not reclassified or replayed. Source-only result requires independent review and Root's fresh matched runtime.
# Root AST timing-fixture recovery — 2026-10-31

The full 284-case infrastructure run at cbb16c7 retained one NameError in the extracted four-phase timing fixture: its injected namespace omitted the newly bound `wait_child_goal`. The fixture now injects the same original failing callable under that name, retaining the RuntimeError and `finally` end-stamp assertions for all four actual blocks. No runtime helper, observation deadline, retry count or product behavior changes. Actual rerun and independent review remain required; original failed log SHA-256 caf597c0ad7c24bd09f8b626500f5ec6d66f79eb9ac7e39a26a006b0352bc7fe is retained.

