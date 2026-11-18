# Evidence

The initial candidate was caf445487fd622b3ae71f7a86e0b7ea570332e39. The final independently requested correction was tested separately below. Base and target are b7f7e3e190fe4fe03b357cdec0958b65e7e13fa9. The evidence manifest records the exact Git commit and tree IDs, Python interpreter, log digests, Git blobs and byte digests for each source layer.

| Run | Result | Log SHA-256 |
| --- | --- | --- |
| Baseline b7f7e3e1 | 17 passed, 15 failed | adf9032ac30b1a8bbe2c45d0c4d483d4bf0c8a1ca849cc2e86c27550e891c100 |
| Fixed targeted checks | 37 passed | 7c90edd556775101425b5ecfa264b128569c30fec84b1916d2bb7271bd07339e |
| Complete infrastructure | 921 total, 905 passed, 16 existing skips, zero failures | 4275015adc8e8979456334d95c55d71abd95bd53d3c8554e00ff21a17afa06a8 |

All 15 failing baseline test identifiers are present as passing in the fixed verbose log. The original upstream failure archive at source 427a9da remains intact and its 15-failure log hash was independently checked.

The two current Rust fixture files equal the blobs at reviewed reader commit 56c56e9efda3b7bd7d38d559e8ffb1e9bde2ff09. The inverse outputs match, byte for byte, the blobs at predecessor a076fc734cffa3f4d90cf5ce45eaf55a2b1c3c28. The unchanged historical chain outputs 90fe84ccdecccddcdb6b8d0530fcbf29dad8d9390c0ac3bfc7200e93a469aabb for IPC and 149693770c6c6bce56def9b9b8a8fe43f8fc7c0657fb31adef45595b64813e06 for Runtime.

In the initial candidate the boundary recognized the two fixed input forms. The subsequent correction permits only the reviewed schema19 input at both new entry points. The runtime snapshot and SQLite discriminator assertions plus the IPC SQLite discriminator assertion are the three exact schema19 regions reversed. Restoration operates on raw bytes and checks the full schema18 digest before decoding. The previous fixture source reads now feed that validated schema18 text into the old checks. Removing only this read wiring and added imports byte-for-byte recovers every old consumer, including all test assertions, mutation logic and hash constants. All three old helper files and the fixed drift inverse metadata retain their base bytes.

Five additional tests check exact restoration and both old chains, unknown or swapped paths, non-byte input, extra and removed anchors, future versions, moved anchors, BOM, CRLF, oversize and unrelated source changes. These are source-only tests. Rust, daemon, native and CI were outside this verification. Python controllers joined before handoff.

Log archive evidence-logs.tar.gz SHA-256 01322a6579d6dac419ef0edc94095119e0ab44a701fa443f4d7ba5c24553014b. Full log receipts are in EVIDENCE.json.


## Final verification after independent review

Source 1e5b709ad6f50d245616f56f849af149fe970532. Base and freshly fetched target b7f7e3e190fe4fe03b357cdec0958b65e7e13fa9. The initial archive hash refers to the archive committed with the initial evidence in this source. This updated archive retains the complete earlier log bytes and adds the review red and final verification.

| Run | Result | Log SHA-256 |
| --- | --- | --- |
| Exact schema18 disk input control before fix at 67611e5f | 1 test with 2 failed subtests, one per input file | 78b541a9a42cb82bfec514b8ad34699ef2fd73b7755c7e70ea284084a65557c6 |
| Strict current-only targeted set | 38 passed, including both rollback refusals and all original 32 tests | 3d569b7602c7044b6d817ae6221316b8a6d38a03c172ba66dc468467240b687c |
| Strict current-only complete infrastructure | 922 total, 906 passed, 16 existing skips, zero failures | 4d4e61384a20773d4957e0752c61e07109fe262f4e64f4fc0585fbc198318b51 |

Both new entry points require the fixed schema19 whole-source digest. Temporary exact schema18 copies of both fixtures fail at the disk entry point, and the pure inverse also refuses those same exact old bytes as input. The inverse still restores each whole schema18 digest and passes the result directly to the original history helpers. Both historical output hashes, the six source path digests, the byte-identical old helper and consumer checks, and all original mutation logic were rechecked.

Updated total log archive SHA-256 6a8151892548a95a40ff1daf5308b1d8522d92638adaa06f2a523d6ac8122bae. All test controllers joined. Local Python source and infrastructure evidence only. Rust, native, CLI, CI and remaining historical fixture work were outside this slice.
