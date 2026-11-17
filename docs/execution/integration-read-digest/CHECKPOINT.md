# Checkpoint

Implementation is based on fixed499 after the documented normal baseline correction from stale cbf. The only production diff is integration_reads.rs: preserve material-first comparison, compare available next, reuse those two immutable local digests at proof construction. No original test assertion, canonical hash algorithm, authority, witness, clock, budget, fee/refund or outcome code changed.

Format/diff checks passed before local commit. Independent static review and clean-source validation remain pending. Heavy slot has been released. Plan to build/freeze matching normal tools, run existing read/FS/Grant negatives and each original failing integration scenario once, with full tracked-input hashes before/after. No claim of measurable gain or remote timeout resolution.

No push, parent ref update, CI, merge or GUI is permitted in this package. Source equality outside the one file and task evidence directory will be checked at handoff. Original failures and pre-correction history are retained.
