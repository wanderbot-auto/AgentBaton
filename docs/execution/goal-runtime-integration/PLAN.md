# Goal runtime integration plan

Full Design v0.2/130 requirements/WP01–14 remain the goal under AB-START-001. Baseline Data PR14 corrected head37bd032f0e1b2dfa75fc9ff768e85801f1693a99 on protected main18f4261da9285bec5446101d47786839b1020d9c. OWNER001/002 fetched and ACK; no003. No native platform Goal tool is exposed; native collaboration tools are used. No real product Provider/account/model, extra spending/resources, production/signing/license authorization is added.

1. Retain Data PR14 exact CI/macOS gates and historical failures; use reviewed test-only0db observer correction, not old greens.
2. Merge separately reviewed Goal execution UI3daa/ce50163 and prepared actual IPC harness03943e0. Core11 author owns schema/migration/session/runtime and exclusive main Cargo target until full suite/build/immutable candidate handoff.
3. Independently review frozen Core source and actual critical normal/failure/SQL tamper/migration cases. Merge it and fix integration-only schema assumptions explicitly; original semantics and unsafe-path refusals remain.
4. Build frozen composed daemon+CLI and445 UI. Exercise original quiet/command/Goal/editor/Data and new actual complete TaskRequest, same-scope rev2/CAS/dropped response, Data, partial cancel and timeout unknown browser flows. No private Owner action crosses HTTP.
5. Independent harness/composition review; add meaningful native/macOS smoke and product CI through PR; exact7/new dualNative/defaultreleaseaudit/real dualDMG before protected integration.
6. Continue actual dependency/validation/Owner/summary graph, factory/workspace capabilities, NL planning and remaining full design. Bounded existing-workspace Work execution is only a slice and cannot be marked completeGoal/product.
