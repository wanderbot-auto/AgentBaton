# Desktop development

This desktop client uses Tauri IPC and the authenticated local daemon. It has no browser HTTP service and does not substitute fixture data when the daemon is absent.

From the repository root, build `batond` and `baton` with Cargo. Start `batond` for development. In `apps/desktop`, run `npm ci`, `npm run typecheck`, `npm test`, `npm run build`, and `npm run tauri -- dev`. Use the same trusted `BATON_RUNTIME_DIR` for desktop and daemon when overriding the platform default. Never expose that directory or its credential to the renderer.

For macOS packaging, prepare the target-suffixed daemon and CLI binaries under `src-tauri/binaries/`, then run `npm run tauri -- build --bundles app`. The app starts the bundled sibling daemon when its socket is absent. Closing the UI leaves the daemon running. This is an unsigned development build; formal signing, notarization and production release are separate gates. The configured deployment target is provisional until both architectures are built and validated.

Frontend tests mock Tauri responses and prove only UI behavior. Rust host tests, daemon IPC integration, native desktop behavior and real Provider integration are separate validation dimensions. This first chain records local draft tasks, cancellation requests and durable events; it does not dispatch tasks or claim connected Provider capabilities.

Project management uses the same authenticated daemon: import a user-entered source directory, inspect its recorded identity, archive its registry record, explicitly reassociate a moved source, or create a separate managed Git workspace. The daemon chooses all managed destinations and rechecks the expected source identity and HEAD. Importing a source does not run repository scripts or make it an approved execution environment. Archival keeps source files and existing history. Workspace states and missing lease evidence are displayed explicitly; an uncertain creation blocks another creation in the UI. Source metadata reflects import/reassociation observations and may be stale. Task drafts can bind to an active imported project; this binding does not dispatch them.
