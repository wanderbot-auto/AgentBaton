# PR #33 finite MCP late-wait diagnostics

Base `26e659dc1297984d8cbbb09945b445cbd1002e97`. Original Product UI job 136824666701 recorded the exact late-wait assertion without the finite state observations needed to distinguish its branches. Take only the already observed categories and comparisons from the exact first assertion frame. Preserve existing collector, wait, stop, owner and success behavior.

Use the new failure-only v7 envelope. Keep old v4–v6 bytes and validation and restore the previous complete projector bytes before every historical source guard. Test with controlled synthetic traces, schema negative controls and the original infrastructure suite. The separate Linux reproduction stays paused. No hosted rerun or push.
