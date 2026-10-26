# Source baseline integrity

The Design v0.2 text, JSON contracts and templates are imported without product-scope changes. `design-v0.2-manifest.sha256.json` records hashes of the original delivered design package. Preparation-only changes to root instructions and setup files are intentional; the core decisions, requirements and work packages remain unchanged.

The original ZIP is preserved in the conversation deliverables and the local preparation backup. It is not required at checkout: CI verifies immutable baseline content using the manifest. Hashes prove correspondence to the supplied baseline, not publisher authenticity.
