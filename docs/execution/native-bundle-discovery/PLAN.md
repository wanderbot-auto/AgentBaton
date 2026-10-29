# Native helper binary discovery

From PR16 base `c6f6b3c2be18ecc339f1cba6a9603657739e4cd4`, diagnose the release bundler failure without changing production logic or making test controls ship. Use the pinned Tauri CLI source, locked offline Cargo metadata and finite generated CLI inputs. Move only the two debug smoke helper modules outside `src/bin`, preserving their bytes and all feature/release guards. Update the entry's two module paths.

Preserve release audit, Cargo manifests/locks, CI names/thresholds/resources/permissions, production Native/Core/UI and schema. Save the original failure, before/after metadata, source provenance and fixture controls. Freeze source for another reviewer and Root's separately coordinated native validation. No Cargo build, Core/daemon, GUI, push or CI operation in this task.
