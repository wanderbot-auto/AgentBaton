# OWNER-002 · AgentBaton Quiet Canvas UI

## Authority and precedence

The owner rejected the previous UI preview and instructed: follow the uploaded design.md; make the main interface simple with little text; use a more forward-looking interaction model rather than a conventional dashboard; take inspiration from ChatGPT; default to a 2×N card view. The next message explicitly authorized redesign and Codex Cloud implementation.

This instruction supersedes the default three-pane dashboard presentation in docs/02-experience.md and the previous dark/purple HTML preview. It does NOT remove the approved product capabilities, permissions, task-centric architecture, native engine integration, or safety boundaries. Keep the original v0.2 baseline intact; record this UI amendment as an execution overlay. No new spending, production release, credentials or real-provider authorization is granted.

The uploaded style reference contains internal site provenance. Only reusable visual tokens and rules are reproduced here; do not publish its internal URLs, capture metadata, restricted logos or font files.

## Experience: a task canvas, not a dashboard

- Desktop landing page: exactly TWO equal-width columns, unlimited rows (2×N). These are actionable TASK cards, never KPI tiles. Do not expand into three/four columns on wide screens. Less than 760px may use one column.
- One quiet 48px top bar, one lightweight 232px collapsible navigation. At 760–1179px collapse navigation to a 64px icon rail. Main canvas is centered, approximately 1056px wide including padding. At >=1700px it may grow to 1168px, still two columns.
- Home = concise title, a single status-filter row, task cards, and one bottom goal composer. No permanent right inspector, event log wall, statistics strip, charts, long explanatory text, giant hero, decorative illustration or management-console table on the landing page.
- Keep existing navigation semantics: Space/Tasks, Capabilities and History. Project links are contextual. Quick chat remains available without replacing the task organization.
- Card content budget: one short title, one truthful state, one latest artifact OR recent action, executing connection/agent and project, one contextual action. At most two preview lines. Details, host paths, full IDs, budgets, capabilities and timestamps beyond the latest event are secondary.
- Flat cards with 6px radius, 1px hairline, no default shadow. Use a common template; different preview types (diff, file, handoff, approval) fit the same space. No fake completion percentage or purely decorative activity animation.
- Card click transitions to a focused conversation/task page. Default tabs: Conversation, Artifacts, Details. Plans are folded initially. Raw observable events are collapsed, not mixed with conversation. Never display fabricated model reasoning.
- The bottom composer remains the common entry: goal/message, optional file reference, project, execution choice, black send control. Sending a goal creates a real draft using the existing core path; it must not falsely indicate dispatch. Follow-ups do not enlarge authorization.
- Quick chat uses a quiet thread view. Converting to a task previews the selected summary/context and preserves authorization boundaries.
- Capabilities is a secondary page: concise connection and Skill/MCP rows. Compatibility matrix, provenance, configuration diff and runtime evidence open on demand. Do not collapse configured/available/read/connected/invoked into one green switch.
- Approvals open a focused dialog showing the exact recipient, selected data, operation and scope; retain the distinction between approval and actual execution. In-scope automation must not gain new confirmation steps just because this UI exists.

## Visual tokens from design.md

Use shared CSS/theme tokens, not per-component approximations:

| Token | Value |
| --- | --- |
| canvas | #FAFBFC |
| surface | #FFFFFF |
| secondary surface | #F4F5F7 |
| tertiary surface | #ECEEF1 |
| ink / primary action | #0A0A0B |
| ordinary body | #4A515C (cool, never warm gray) |
| muted / subtle | #7A828F / #A4ABB6 |
| hairline / light / strong | #E4E7EB / #E8EAED / #CFD3D9 |
| only brand accent | #0F766E |
| accent hover / soft | #115E59 / #EAF8F5 |
| focus ring | rgba(15,118,110,.14) |
| warning / error | #B45309 / #B42318 (semantic use only) |
| text sizes | 14px / 18px / 24px via tokens |
| text weights | 400 / 500 / 600; business UI never >600 |
| title font | Inter Tight, Inter, system sans-serif, PingFang SC |
| body font | Inter, system sans-serif, PingFang SC |
| technical text | JetBrains Mono, SFMono-Regular, Consolas, monospace |
| radii | 3–4px buttons, 5px navigation, 6px cards, 8px composer/dialog |
| controls | 30px visual desktop height; mobile touch target >=44px |
| icons | consistent 1.5px line icons, 16–18px |
| feedback | 160–240ms; obey prefers-reduced-motion |

Primary actions are BLACK, not teal. Teal signals focus, active execution and connection evidence only. Do not add purple/blue gradients, glass, blur, broad colored backgrounds, floating shadows, oversized round cards or pill buttons/tabs. Normal cards have no shadow. Restrained dialog shadow is allowed.

The source's dense table/control-plane layout is intentionally NOT copied to the homepage: the owner's explicit simplicity and 2×N instructions override that layout, not its palette/type/material system. This reference is light only; do not invent an unreviewed dark palette. This amendment does not silently delete any existing product requirement for future supported appearances.

## Exact executable reference

This directory contains index.html, styles.css and app.js. Open index.html, or serve these local files with an ordinary static server. No dependency installation or network fetch is needed. It contains synthetic examples, NOT a live backend. Do not import demo records or simulated success handlers into production.

`prototype-evidence.json` records local browser interaction/render checks only. It is not evidence of native macOS, actual provider or product testing. Native-app delivery must still follow the normal execution runbook.

Port the structure and tokens to the existing React/Tauri components; do not embed the prototype as a second application or rewrite the daemon to fit mock data. Split components around Shell, TaskGrid, TaskCard, GoalComposer, TaskThread, ArtifactPreview, CapabilityList and ContextDialog as appropriate. Reuse existing domain types, API calls, state/error semantics and tests.

## Implementation and coordination

1. At the next safe checkpoint, fetch latest protected main without resetting local work. Read OWNER-002 in docs/execution/OWNER_INBOX.md and ACK it in the active execution STATUS/PROGRESS with the exact commit read. Do not claim an ACK before actually reading it.
2. Preserve current foundation work and fixes. Use a dedicated UI branch/PR based on a compatible foundation commit; resolve documentation/manifest changes by retaining both sets. Never replace another worker's uncommitted files.
3. Have one UI implementer own shared front-end files; a separate reviewer checks visual fidelity, interaction semantics and real-core wiring when native subagents are available. Main agent handles integration. Do not invent subagent/Goal availability.
4. Build the light card homepage and composer first, then the focused task view, contextual panels and secondary pages. This is implementation order, not reduction of the complete product goal.
5. Every live control must invoke a real supported operation. Loading, empty, disconnected, permission-denied and failure states use the same shell. Disable unsupported actions with an explanation instead of simulating success.
6. Preserve protected-main PR and required checks. Keep product tests separate from the supplied prototype checks. Return actual screenshots, changed files, exact commit/PR, implemented versus blocked interactions and remaining scope. Then continue the other approved work.

## Acceptance checks

- At 1488×981 and 1440×900, default is two columns, no permanent right inspector, no KPI strip, no raw event dump. Scrolling may reveal additional rows; composer does not cover the last accessible card.
- At 1024×768, two columns with collapsed navigation; at 390×844, one-column fallback, no page-level horizontal overflow. Do not hide critical permission/state information solely to fit width.
- Tokens match the table; computed business font sizes are only 14/18/24 and weights <=600. No network font dependency or bundled restricted font assets.
- Real draft creation, task filtering/open/back, real error state, artifact preview, selective approval, quick-chat-to-task and contextual capability evidence work with their actual API contracts. No prototype data in the normal product path.
- Keyboard focus is visible; dialogs trap/restore focus; Escape closes them; inputs and actions are labeled; status is not color-only; reduced-motion is honored. Command search uses the project's existing shortcut conventions.
- Provide home and task screenshots from the actual implemented desktop/frontend. Browser-rendered prototype screenshots must not be presented as native-app evidence.
- Report OWNER-002 ACK and the UI implementation PR. The message being committed is not evidence that Codex has received or implemented it.
