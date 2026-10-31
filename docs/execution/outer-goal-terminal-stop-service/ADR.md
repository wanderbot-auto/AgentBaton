# ADR: Service a committed owned terminal hint at every existing Supervisor service boundary

Use the existing hint and complete fresh stop-only policy immediately after the outer Supervisor tick. The hint remains a scheduling observation, never authority, acceptance or a stop/join receipt. The mandatory full Goal sweep still runs at its original position; ordinary completed sessions without a Goal hint stay available.

The initial test design proposed dropping `goal_graph_jobs`. Source tracing found the fresh stop policy also counts that table, so dropping it would test the wrong boundary. Instead inject one clearly marked private-fixture prepared row whose SQL relationships are valid but original strict CaptureOrigin document is malformed. The fresh stop scan can count it and validate the original node; later graph worker decoding must refuse it without starting work. Actual tests will prove the claimed order, error and cleanup; static inspection alone is insufficient.

The four-second budget refusal is preserved. Removing a response gap is independently useful but does not establish that it caused a particular total elapsed measurement or guarantee a future CI result.
