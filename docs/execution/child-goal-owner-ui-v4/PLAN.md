# Owner UI state observation v4

Base: b3b3426efe9ac2f84a80daafdbe751cf97b85c80, isolated branch `codex/child-goal-owner-ui-v4`.

Add finite read-only renderer state at the two endpoints of the existing Owner refresh observation. Preserve all business actions, permission gates, timeouts, polling limits and cleanup. Keep v1/v2/v3 compatibility and default v3 recorder unchanged. Add pure negative/source-inverse controls and separate mounted deferred-tail/epoch controls. Root performs actual mounted/type/build/browser/CI checks after independent review.
