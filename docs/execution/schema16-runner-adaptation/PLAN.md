# Schema16 runner adaptation

Baseline `7d7b7016295a93a4f7ba5fe0e4e320f197e3b530`; requirements AB-SEC-007 and AB-PRJ-002/003/005 current-version validation. ACK OWNER-001/002; full130 product scope and Designv0.2 are unchanged.

Decision: migration16 changes Task workspace semantics without adding Factory tables. Current runner candidates/live databases must equal integer16; old15 and future17 are refused, as are float16/bool/string versions. Keep the exact12 Factory tables and all independent typed counts, zeros, sequence, cleanup, permissions and resource limits. Never relabel a genuine14/15 generator/input.

Approved13-file list and precise substitutions are in portable `CHANGES.json`. The Root-owned Factory runtime/planning support scripts and Native Rust remain separate integration work. No production/schema/IPC/CI changes are included.
