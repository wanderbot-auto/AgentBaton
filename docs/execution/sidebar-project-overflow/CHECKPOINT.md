# Sidebar overflow checkpoint

Source commit25e9fbeda9fab2043f5b167fea86fc7e48d5a8d3, CSS SHAd1504ff61459f8d865e3d89abde17b6fd14c6b16855fa3bb1bc38b2e25252963; only .sidebar-projects grid-template-columns:minmax(0,1fr) differs from baseline822. Actual old build/pass38 then expectedoverflowFAIL1 and new build/pass39/28images. Sidebarscroll247→231(client231), track235.078→207, existinglabel185→157 with ellipsis, full title/name and working project selection. Folded rail63/63 unchanged. Main width and document overflow unchanged.

Evidence directory/worktree: /workspace/.onboarding/sidebar-project-overflow and /workspace/.worktrees/sidebar-project-overflow. External verifier02140a19ac0a0334eba05f81cc6635ed2ecaca18f63d02235397c9df97f99bc6. Local manifest SHAd8e01ce9b5239216c8a816d208992024f32a397379ae3bf017b3e027c312ea9d. Original source harness untouched. The first verifier mobile-locator error and original correct old overflow failure remain retained.

No Cargo/target/browser/daemon writer remains. Exact own Node symlink removed after builds; no dependencies installed. Next: independent reviewer actual source/build/layout check, then primary integration and fresh exact native/macOS CI. Do not describe this Linux browser fix as tested native macOS. No permission or production/account/credential change.
