# Native Grant contract fixture correction

Base `fb7ff7d861872eec18e010f7b64a0ead63740faa`; branchcodex/native-grant-contract. OWNER001/002 and existing development limits retained. Scope is the debug-only Owner harness/controller, host runner predicate and their tests. No Core, NativeParent, approval formatter/permission, schema, Cargo/lock, CI, STATUS or threshold change. ReqSEC003/010 UX003 TSK003/007/008 partial; no full acceptance claim.

ADR: bind role identities by actual Core semantics. policy.rs creates Grant.id and Grant.approval_id as independent opaque UUIDs; sessions.rs stores the actual Grant as the Owner proposal result, marks the proposal consumed/revision+1 and records approval.confirmed. A field name does not imply Grant.approval_id==proposal.id. No invented inequality is required either.

The accurate test chain is: exact readonly DB Grant == saved proposal result == native response result; single execution_grant.approved event at baseline+1 has matching Grant aggregate/revision/source and complete payload(grant/project/root/approvalID/scope/enforcement); single original proposal approval.confirmed at baseline+2 has exact source/revision/consumed payload. Grant ID/approvalID are nonempty/bounded opaqueUTF8 strings, complete approved scope/refs/enforcement remain exact. Existing19zero schema12/CAS/sequence and every page/screen/capture/permit/cancel/cleanup gate stay mandatory.

Independently confirm production source and original doubleMac packets; reproduce original rejection; correct only test assumptions, add a real40322 Core contract fixture plus pure negative controls, freeze source and evidence for independent review. Root owns safe Cargo/sidecar restoration and newMac CI. No private Provider/account/model call or production publication.

## NG-FBFD-01 typed JSON follow-up

Independent fixedfbfd review found Python dictionary equality accepts true==1 across independent JSON records. Add a small recursive JSON-value equality that requires exact Python JSON types (including bool/int/float), object keys, nested values and array elements; reject nonfinite floats. Use it for complete DB/result/response/event payload and snapshot/binding correlations, and typed page revision/index/count. Preserve every original test and screen/capture/CAS/19zero threshold. Break synthetic aliases by actual JSON encode/decode before controls, matching independent persisted records. Rust requires no change.

## Scalar-CAS completion

The fixed1839 typed-record review found a remaining plain confirmation.revision comparison. Enumerate the numeric/boolean conditions in pending_error, approved_error and presentation_error, use exacttyped comparison for schema12, revision/sequence correlations and reference metadata, retaining all existing explicit int/boolean checks. No broad data/schema/production refactor. Grant event revision already had an int guard; preserve it and add explicit bool/float regression rather than pretending it previously failed.
