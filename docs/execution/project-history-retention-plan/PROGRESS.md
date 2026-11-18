# Progress

Clean base ec2ca48b12241451905157240238987c821e2b2a and four reserved paths
confirmed. AB-START-001, D04, project-policy schema and parallel-development
rules checked. Scope passed. The ACK route returned a transport error; the
workspace receipt records the ACK.

The public Core API and first implementation are in place. Policy and body
capture are separate. Stable references carry just kind, ID and decisions,
and the plan computes from synthetic or caller-verified snapshot metadata.

The public-entry tests passed at c7e672cd2d92edfa232656c0cab1328aff86565f, including exact day boundaries,
capture opt-out, active and uncertain state protection, same-project lineage,
conflicting owner snapshots, time overflow and deterministic preview metadata.
Locked offline Clippy and workspace formatting passed.

The existing history pagination suite had 12 passes and one failure, and the
schema18 suite had one pass and two failures. Isolated fixed-base execution
reproduced the same tests and exit codes. All four red runs are preserved.
Scope and integration merge preview passed. Evidence is ready for independent
review.

Leader independently accepted `a91450f4a47b3f154b8a4286de403ec0c0a544d0` and authorized the four-path
handoff. This follow-up records the decision in the task documents.
