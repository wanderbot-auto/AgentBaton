# Project-binding fixture timing progress

Actual Intel job121921608984, Product40422958570, failed App.test.tsx:27 in5164ms against unchanged5000ms bound; suite48pass1fail. Actual checkout0c1663a is the PR merge of130f into c4e72c6. Raw log copied without rewriting and hash-recorded. No step-by-step timing or localMac reproduction is claimed.

Original single case passes locally. Source4e18b723 waits for the real enabled active-project option, selects and asserts its ID, then uses actual focused userEvent paste and asserts exact goal text. Original archived-disabled and api.create(goal,key,project.id) expectations remain. All other11App test bodies are byte-identical, including the two remaining user.type task-creation/idempotency cases. No timeout/config/production change or assertion removal.

Actual targeted App count is12 (not5); all12, full49UI, typecheck and build pass locally. Tests mock Tauri transport, so no real browser/native/Mac acceptance is inferred. Only worker-created exact node_modules symlink was used against existing locked Root dependencies and removed after checks; real installation untouched. Independent delta review, parent integration and fresh exacthead CI remain pending.
