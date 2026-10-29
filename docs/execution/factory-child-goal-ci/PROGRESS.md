# Progress

- Added three explicit real-run child Goal cases after the unchanged existing child-planning call. Each receives the same planning manifest, built frontend, schema15 tools and Chromium; each writes its own factory-child-goal case directory.
- Scoped nonzero collection attempts all three child Goal paths and raises all failed case/exit pairs. Original Factory failure collection/raise and original check=True child-planning call remain intact before this block; they are not caught or hidden by the new handler.
- YAML, inline Python AST and Bash syntax PASS. Every old acceptance AST node remains an exact prefix; all other jobs/existing step metadata and20-minute budget are unchanged.
- Pure CI orchestration controls PASS for all-success and all-three-fail scenarios, with mocked subprocess calls only. Actual browser, Cargo, harness success and CI timing are NOT_RUN. This does not add product verification evidence.
