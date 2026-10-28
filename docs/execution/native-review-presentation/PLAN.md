# Native complete review presentation

Baselinee3ccf86a8b12e7ed1371074de095bb454c0b62aa; branchcodex/native-review-presentation; OWNER001/002 acknowledged. ReqSEC003/010 UX003 TSK003/007/008 partial. Scope: central native_parent production display, its own-process OS harness/controller, Python host runner/controls and these4records. No Core/schema/permissions/UI/CI/lock/dependency change.

Actual ARM evidence at /workspace/.onboarding/pr16-e3cc-mac-review is immutable: expected body3132B, actual2880B, title1/message0, sheet520x1329 y−614 on_screenfalse. The per-line-leading-whitespace relationship is an observation, not proof of which layer changed it. Exact guard correctly refused all clicks.

ADR: display the complete original message as bounded sequential reversible JSON-string rows. Every visible row has a non-whitespace ordinal prefix; original indentation/linebreaks/controls are escaped explicitly; Chinese and emoji remain directly readable. At most40 encoded UTF-8 bytes/row,8 rows/page; no original bytes omitted. Verify full roundtrip before opening page1. Keep the original native title and caller-owned lifecycle for every page. Every page requires genuine OK; any Cancel or parent invalidation stops. After all content pages a separate final display confirmation is required. Existing Graph continuation/final confirmation semantics and Core checks remain additional authority gates.

The presentation envelope accepts the existing1MiB snapshot plus32KiB fixed explanatory envelope. It does not lower16KiB or1MiB snapshot limits. Finite local rows/pages bound layout; total memory is bounded by this envelope. No clipping/normalization/summary/digest replaces full text.

OS harness: inspect/capture/permit each exact content page and final page, then real Cancel on the final page; repeat same pending proposal and approve all pages+final. Host observes original19 authority tables zero and sequence unchanged before every click; only final approval may create exactGrant+twoevents. Existing500ms sheet stability,4s owned capture,75s harness and85s runner bounds stay unchanged. A different live sheet number is required after each click; failure to obtain one fails closed.

First static/meaningful Python controls, then Root-safe native cache window/independent review, then real new dualMac CI. No Linux/mock evidence may claim AppKit success.

ADR readability refinement: ordinary Chinese/emoji remain literal standard UTF-8 JSON text; only JSON/control/bidi characters are escaped. Byte bounds split only between Unicode scalars. The initial all-ASCII experiment was rejected for bilingual readability and remains intermediate evidence, not the final design.
