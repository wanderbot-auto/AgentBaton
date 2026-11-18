# Project history retention preview

Implement D04's adjustable 90-day engineering default, project TTL overrides,
permanent retention and a distinct setting for newly captured body text.
Compute from trusted snapshot metadata using a fixed Unix millisecond clock.
Keep non-detail categories, pins, active/unknown references, and missing or
future timestamps. Include a required gap reason and source-specific index or
cache IDs in each due detail. Ignore other projects in the response and reject
ambiguous identities or lineage before returning a plan.

Reuse existing project, task, conversation, session, ID and Core error
semantics. Add the public module, planner, and external integration tests only.
Run locked offline targeted and existing history regression tests with synthetic
data. Validate formatting, verify Git scope and preview, preserve evidence, and
hand off to leader for independent review.
