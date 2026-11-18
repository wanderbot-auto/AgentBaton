# Integration browser candidate schema 19

Fixed base 98413ca1f6e1006a74d5c6ff7f40494b0509da7c, branch collab/host19-browser-candidate.
Carry forward the original CLI refusal from the preceding driver slice.
Update only the live candidate manifest version check. Add a strict one-anchor
inverse before the pinned two-anchor runtime layer and older historical layers.
Keep the current source, tool, runtime permission, frontend, and timeout checks.
Run the schema, historical, and full infrastructure controls. Build the locked
sidecars and frontend from a clean commit, verify them and the local runtime
files, and attempt the existing normal and command-failure CLI cases. Record
the boundaries reached and retain raw failed-case evidence privately.
