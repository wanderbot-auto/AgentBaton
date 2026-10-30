# Three-round browser lifecycle fixture correction

Requirements: AB-VAL-009, AB-PRJ-004, AB-PRJ-005; approved Design v0.2 slice under AB-START-001. Base `8a6708e63fb22cb41178310451b19861ff2ceed1`.

The actual three-round browser fixture requested a fresh UI start while the independent Core watcher already held active integration work. Its context/target reads honestly returned WORKSPACE_BUSY, keeping the UI start disabled. Preserve that product guard. Observe the watcher through the original bounded read/finished loop first, then refresh the real UI and require its explicit same-run check. Do not infer that the UI created the run.

Keep the exact original observation call, gate health, stage/Command/Grant bounds, false/false/true receipts, seven settled stages, eight charged attempts, sole CAS, source preservation, same-run result and seven original screenshots. Add a pure order control that rejects the original order and retain the real failure without replaying its unknown outcome. Root must independently review, rebuild and freeze the exact new HEAD, and run the real browser; these source/pure records do not prove runtime success.
