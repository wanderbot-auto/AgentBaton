# Schema14 development installers

These are CI development artifacts from Product [40840944666](https://github.com/wanderbot-auto/AgentBaton/actions/runs/40840944666), built at `c69b3aad59d8379c1dbd2a2cd2ac9cef937cffce`. Its tree matches protected merge `653679db60d5c68f7e3c5b5e146ef58aed10de23`. They are not a production release.

| Architecture | Download | DMG SHA-256 | Signature |
|---|---|---|---|
| Apple Silicon | [artifact12884860971](https://github.com/wanderbot-auto/AgentBaton/actions/runs/40840944666/artifacts/12884860971) | `8f2647bd1dc19f4646a66c7c6d5dcc338ad9f1eeb6bdfe8699bd738ed640e417` | ad-hoc, no Team ID |
| Intel | [artifact12884398731](https://github.com/wanderbot-auto/AgentBaton/actions/runs/40840944666/artifacts/12884398731) | `45ed4293ed08e1ced22947b5c7c891c33a549d24e7ff5d2491ca0f3c5503237b` | unsigned |

Official artifact ZIPs, checksum/build records and manual checklists were retained and independently checked; the hashes above refer to the DMGs inside the ZIPs. Neither artifact is Developer ID signed or notarized. Real macOS Core/native WebView/Owner dialog tests and default release-control scans passed separately. The packaged default application has not been mounted, installed or manually launched for acceptance.

Manual acceptance remains unchecked: verify architecture and DMG checksum; inspect build/signature records; install under the machine's ordinary security policy; confirm task-card home and focused task flow; verify Native Owner approvals/private database and cancellation; check UI-close/background behavior; verify local data/history and uninstall behavior. Do not disable OS security. Live Provider authentication/tests require separate authorization.

Schema15 child Goal execution and workspace creation are not included in these schema14 packages. Full Design v0.2 is still in progress.
