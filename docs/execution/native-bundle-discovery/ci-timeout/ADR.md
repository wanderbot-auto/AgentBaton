# ADR: bounded desktop job allowance of60 minutes

Status: authorized candidate, pending independent review and a new exact CI run.

The original c6f6 Intel job123175557621 exhausted the existing45-minute job limit during release compilation, after native readonly/Owner checks passed. Its official check annotation states `The job has exceeded the maximum execution time of 45m0s`. ARM job123175557753 has a different observed failure: default bundling cannot copy a phantom helper binary. Preserve both outcomes; increasing job time alone cannot repair ARM's failure.

The owner-authorized engineering decision is a15-minute extension to60 for the existing desktop matrix job. Keep all standard hosted runners, matrix, steps, tests, checks, release audit, artifacts, dependencies, action pins, permissions and individual product-test limits unchanged. Existing CI quota remains the resource boundary; no purchase, account switch or new resource is authorized. Root reports the repository is public based on its read-only API evidence; this leaf did not independently repeat that lookup.

The extension allows bounded completion time for the existing sequential native/default-release/audit/packaging job. It does not claim compilation is faster, guarantee60 minutes is sufficient, or convert cancelled/failed evidence to success. Verify the next exact CI result before claiming a package or completed gate. The helper relocation is a separate source commit and remains separately reviewed.
