# Child-plan read cycle and explicit submission outcomes

Base033a7b2918c5af0637f7381c47b7a0c49d1afc63; branch codex/child-plan-read-cycle, isolated sparse checkout with unmodified docs tar.gz excluded. Requirements AB-TSK-004/008/011 and AB-SEC-001. AB-START-001 scope; Root explicitly approved this narrow product change after read-only proposal. ACK OWNER-001/002. No Core/IPC/schema/provider/budget or execution-permission change.

Preserve every original parent polling cycle and full client/parentReady/authority/CAS gate. Make the cycle explicit independently of object identity, discard obsolete read replies, and avoid a second read chain for equal-valued object clones. Retain read-only descendant provenance and historical exact-key recovery. Preserve submission diagnostics through unrelated reads, and identify an intent that sent no new request without automatically replaying it.

Source author owns only FactoryRuntime.tsx, FactoryChildPlans.tsx, focused component tests and this directory. Root owns independent review, Node/type checks, actual IPC/browser and CI. Source author executes static diff/scope checks only, with all runtime results not_run.
