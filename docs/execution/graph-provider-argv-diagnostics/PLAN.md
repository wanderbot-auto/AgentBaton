# Graph Provider argv failure diagnostics

Requirements AB-TSK-008/009/010, WP08, AB-START-001. Baseline PR18 `71fd739`. Keep the complete approved product scope; this is one partial diagnostic slice.

The current failed UI summary identifies the Graph argv assertion but cannot distinguish empty, oversized or missing-NUL argv. Preserve its refusal semantics and publish only a finite reason plus whether the record had already been bound. Do not add recovery, retries, cancellation/stop inference, IO, permissions or time.

Use explicit finite-diagnostics version2 with an exact new field and preserve strict legacy version1 records. Test original guard equivalence, first failure retention, private-data rejection, legacy/new schema inverses and failing publication. Root then independently reviews and executes actual matched browser/Core/CI validation.
