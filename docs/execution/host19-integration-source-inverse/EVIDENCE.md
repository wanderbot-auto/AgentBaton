# Evidence

The validated source is caf445487fd622b3ae71f7a86e0b7ea570332e39. Base and target are b7f7e3e190fe4fe03b357cdec0958b65e7e13fa9. The evidence manifest records the exact Git commit and tree IDs, Python interpreter, log digests, Git blobs and byte digests for each source layer.

| Run | Result | Log SHA-256 |
| --- | --- | --- |
| Baseline b7f7e3e1 | 17 passed, 15 failed | adf9032ac30b1a8bbe2c45d0c4d483d4bf0c8a1ca849cc2e86c27550e891c100 |
| Fixed targeted checks | 37 passed | 7c90edd556775101425b5ecfa264b128569c30fec84b1916d2bb7271bd07339e |
| Complete infrastructure | 921 total, 905 passed, 16 existing skips, zero failures | 4275015adc8e8979456334d95c55d71abd95bd53d3c8554e00ff21a17afa06a8 |

All 15 failing baseline test identifiers are present as passing in the fixed verbose log. The original upstream failure archive at source 427a9da remains intact and its 15-failure log hash was independently checked.

The two current Rust fixture files equal the blobs at reviewed reader commit 56c56e9efda3b7bd7d38d559e8ffb1e9bde2ff09. The inverse outputs match, byte for byte, the blobs at predecessor a076fc734cffa3f4d90cf5ce45eaf55a2b1c3c28. The unchanged historical chain outputs 90fe84ccdecccddcdb6b8d0530fcbf29dad8d9390c0ac3bfc7200e93a469aabb for IPC and 149693770c6c6bce56def9b9b8a8fe43f8fc7c0657fb31adef45595b64813e06 for Runtime.

Both test boundaries recognize only their two fixed digests. The runtime snapshot and SQLite discriminator assertions plus the IPC SQLite discriminator assertion are the three exact schema19 regions reversed. Restoration operates on raw bytes and checks the full schema18 digest before decoding. The previous fixture source reads now feed that validated schema18 text into the old checks. Removing only this read wiring and added imports byte-for-byte recovers every old consumer, including all test assertions, mutation logic and hash constants. All three old helper files and the fixed drift inverse metadata retain their base bytes.

Five additional tests check exact restoration and both old chains, unknown or swapped paths, non-byte input, extra and removed anchors, future versions, moved anchors, BOM, CRLF, oversize and unrelated source changes. These are source-only tests. Rust, daemon, native and CI were outside this verification. Python controllers joined before handoff.

Log archive evidence-logs.tar.gz SHA-256 01322a6579d6dac419ef0edc94095119e0ab44a701fa443f4d7ba5c24553014b. Full log receipts are in EVIDENCE.json.
