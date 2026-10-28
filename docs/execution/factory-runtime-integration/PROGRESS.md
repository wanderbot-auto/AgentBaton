# Progress

Source composition489 and tested334 are local, not a new PR. Native first run compiled all13 previously unrun Factory UI tests (70 library total) and six Owner controller tests; readonly29 passed/one fixture insertion failed at the actual schema13 CHECK. The narrow one-line test metadata correction retains all drift assertions; independent review closed it. Second run106 passed, format/Clippy alltargets succeeded, tracked sources unchanged. Original failed stdout and second success are preserved losslessly inside the manifest-bound archive.

Root overlooked tauri-build copying old sidecars into shared target/debug while using them as macro resources. The oldfc5/906 tools were then incorrectly frozen as053 by the author; their label is rejected and original manifest retained. Core is rebuilding/fixing tool provenance. This does not prove a current Factory executable or GUI; Native tests use the compiled library and no shared debug tool execution is claimed.

Core053 independent static review found FD053-01 elapsed accounting early-return omission and retained the known cooperative restore cancellation gap. Later author fixes/tests are in another moving tree and do not close this fixed-source checkpoint. New browser driver is under independent review and has not established any runtime pass. No remote push, formal approval, merge, Mac run or installer has been performed on this Factory branch.

Composed source72c84ef Node643/41, typecheck and actualbuild passed; three assets hash-bound, tracked sources unchanged and temporarynode_moduleslink removed. Logs/command runner preserved in NODE_COMPOSITION_EVIDENCE.tar.gz. No browser/daemon test was run.
