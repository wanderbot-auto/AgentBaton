# Static checkpoint — runtime not run

Source commits: `72c39c8d13bcf429b41e9226488ac2c07c3cbf82` (initial standalone harness), then final `49bcf16b15968fe5b81961852df1fa12dd406ffc` (complete original text comparison). The final script SHA is `6100ba1d3aecab214fe31c9537a2b8adcb9f90be51640e7364773e48049273df`. Base is exact `ead1f468546694e0852205a66416432d8be1a4c2`. No previous Goal Owner/UI/Core source is changed.

Executed with the current Python interpreter only: `python3 -m py_compile`, new harness `--self-check`, `--fixture-self-check <actual-r7>`, and `git diff --check`; all0. The same three script/support SHA values were recorded immediately before each command and again afterward. No listener, subprocess Provider/daemon/cc, browser, Cargo, Node, account/model, remote or approval transaction was started by these commands. Generated ACP is compiled as Python syntax, not executed.

Prepared runtime checks CP01–CP09 are **not_run**. The harness imports Playwright only in runtime mode. Running with no mode writes `NOT_RUN.json` and returns2. `--run` requires Root-provided `--tools`, `--frontend`, `--manifest`; output is exclusive/private, and actual tooling remains immutable. Root supplies a manifest with integer `schema:14`, `source_head`, `compiled_core_source_head`, exact `binaries:{batond,baton}` SHA strings and `frontend_assets:{relative_path:sha256}`. No schema13 fallback. Example command (not executed):

```
python3 apps/desktop/tests/factory-child-planning-real-ipc.py --run \
  --tools /workspace/.onboarding/factory-child-planning-core/private-tools14-current \
  --frontend <Root-frozen-schema14-dist> --manifest <Root-bound-manifest.json> \
  --output <new-exclusive-runtime-evidence-directory>
```

Expected tooling supplied by Core author: batond `64d0a247a2eb15d4fe194cc6d5e34bf26bdf88e5fb73bf2b33763396760d2c05`; CLI prefix/suffix `f25c9356…8dda9` (runtime requires the full exact Root manifest; no abbreviated hash is accepted). No tooling was executed or overwritten here. Core compiled source is `92658e4cecd74517371aaf47c5ce0f7de38e0eb0`; parent confirms production equivalence with composed base, while the actual run must bind its own exact manifest.

Next: independent fixed-source review, Root fresh built assets and exclusive actual browser window, then one actual run retaining all failures. Failures must be fixed by diagnosis, not product timeout/cap increases or relaxed stop checks. Root full Rust work currently owns the shared target; this unit has never acquired or used it. No local daemon/browser/helper remains from this static phase.

Not established: browser end-to-end, actual Native Owner GUI, macOS either architecture, real model/provider, optional external-validator policy UI path, child Goal execution, new workspace authority, TaskAccepted, full failure matrix or complete product acceptance.
