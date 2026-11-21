# Progress

The prior and cancelled Rust workflow blocks are byte-identical. Workspace tests took 681 seconds on the last success and 930 on the cancelled run. The schema 14–16 migration aggregate rose from 246 to 327 seconds. The current job entered the real schema 18 step after 1,391 seconds. The last successful run used 244 seconds for that step and 33 afterwards. Applying those observed remaining durations to this start gives 1,668 seconds, with 132 seconds to spare under the new 30-minute limit. This estimate leaves actual new hosted timing open.

The selected candidate changes one YAML value and adds only the exact backward source mapping and guard controls. The global Infrastructure timeout validator stays unchanged. Original evidence and both prior run dispositions remain intact.

The actual YAML has exactly one changed field. Its exact inverse matched the preceding Git blob and the original packaging and historical inverses produced the same result. The original environment workflow and infrastructure validator were checked byte-identical. The validator admitted 20 pinned Action calls. The initial negative test mistakenly expected a filepath in the unchanged validator output. It already rejected 31 minutes and the corrected test now checks the actual error contract. The initial log is retained.

The final 34 focused tests and complete suite passed. There were 1,132 passes and 16 pre-existing skips. Source is frozen at `244399a43da7792e36fb68e9738b39bc43bb9c68`. The existing hosted macOS chain continues independently and the changed Rust budget was not used by that run.
