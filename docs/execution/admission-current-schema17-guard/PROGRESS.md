# Actual progress

Root reports exact ba4612a Product41154588518 Rust failure at admission_performance.rs:105: current Store returns17 while fixture expects16. Source inspection confirms the fixture calls Store::open rather than constructing a historical16 database.

Only the typed current-schema assertion changes16 to17. A pure byte comparison proves the entire unique #[test] body and every other fixture byte are unchanged, including real supervisor launch, drift rejection, deadlines, stop state and budget assertions. Rustfmt --check and git diff --check pass. No actual admission execution or CI rerun occurred in this source task.
