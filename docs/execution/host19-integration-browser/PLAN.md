# Integration browser driver schema 19

The branch is collab/host19-integration-browser. Fixed base cde501db847d1bbfc27a298e16cd772afc1a41e9.
Verify the incoming Core schema using the real private daemon and SQLite.
Reproduce the two original driver refusals before changing the source.
Update only the two runtime version checks and wire the four historical readers
through a fixed-byte inverse. Verify the inverse against the original Git blob.
Keep the current CI schema probe, preflight, projection and source attribution
independent of the historical inverse. Preserve refusals and rerun targeted,
historical and full infrastructure verification. Report actual browser/CI scope.
