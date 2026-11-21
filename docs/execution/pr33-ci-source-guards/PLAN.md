# Exact PR #33 macOS partition source guards

The task worktree starts at contract commit 7c269ebf1d122a8b02541da894d0655b88b6756c. The workload uses integration base 9e3add27c37f4022dcd2517ab9d1c9cc8f3e9102. Frozen candidate 32950beb9c8a297a80d19474c00a2a501180c01e has tree 7ad0888e659a8a24323f81957413dd173d316609 and product workflow digest 854c9502d344002ff81535d62a002c9af63c96dbb5d01ea167c7724699f39b57. It supersedes the initial candidate cf609efced9714c238b10893689939c211c72003.

Verify the actual four-job Product graph and provenance checks against that input. Run the real first-step shell and the scope/receipt anchor with offline fixtures. Check each official Action by job, role, pinned SHA and inputs. Invert just the reviewed workflow partition, then pass the restored bytes through the original historical guards. Run full infrastructure tests from a private combination of the frozen A and guard commits.
