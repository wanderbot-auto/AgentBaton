# Fixed Apple Git entry point with a protected-volume proof

## Observed incompatibility

Product run `41169977095`, ARM job `124445631271`, retained at `.onboarding/root17-accc-ci/arm-product.log`, reported integration 5 pass / 15 fail. Context errors were `INPUT_CHANGED`, phase `integration-read-phase:worker-system-git-pin`. The diagnostic observed PATH-first Homebrew Git owned by UID 501. Apple `/usr/bin/git` was canonical, root-owned and executable, but had 78 links. These observations explain why the former source/leaf policies reject the available candidates; they do not establish that every integration failure has no other cause.

User-owned PATH Git remains untrusted. The generic leaf policy remains single-link. System-image hard links are a macOS system-volume implementation case that the generic mutable-file policy did not support.

## Program source and actual execution

The API has no explicit caller-selected Git executable. On macOS, Core pins and the workspace library's private `git_command` both select the shared constant `/usr/bin/git` directly. This is a default source policy, never a failed user-program selection followed by fallback. Absence, aliases, unavailable proof or execution errors fail closed. There is no Homebrew fallback.

The existing `source_capture` and `adoption` production paths already delegate to the private workspace command builder, including bounded controlled calls. Changing that common builder covers actual preparation, capture, adoption and CAS commands without a new program-manager abstraction or an `AdoptionControl` argument. Those algorithms and their sources are unchanged. Linux keeps its prior PATH selection and local Git argv policy.

The shared child-only environment helper removes `GIT_*`, developer/SDK/toolchain selectors, `XCRUN_*`/`xcrun_*` selectors, `DYLD_*`, `_DYLD_*`, `__XPC_DYLD_*` and `LD_*` injection variables. It handles inherited and explicit command variables. macOS children receive only the standard system directories in PATH. Existing fixed Git environment settings are then applied. No process-global environment, system permissions or active Xcode configuration is edited.

## Narrow Apple leaf exception

Only the exact canonical spelling `/usr/bin/git` can use the Apple capture entry point. A held descriptor must identify a regular executable owned by root, with no group/world write permission, positive link count and the same device as the volume proof. Both `fstatfs` and `fstatvfs` describe that held descriptor. The volume must report readonly through both APIs, APFS, local/rootfs/snapshot mount flags, root mount owner, mountpoint `/`, and a bounded `/dev/disk...` source. `NOEXEC` and ignore-ownership mounts are refused.

Each of `/`, `/usr`, `/usr/bin` is opened component-by-component using the existing `NOFOLLOW` directory primitive, must be a root-owned non-group/world-writable directory, and must match the leaf's complete captured volume facts and device. Held ancestor descriptors, named ancestors, the held executable and a reopened `NOFOLLOW` executable are all rechecked. Full file stamps, native flags, volume identity/flags/source, digest and the original link count remain pinned. A link-count change, even 78 to 1, is a change. No mutable file, writable volume, alternate path or unknown volume proof receives the exception.

The observable readonly local root APFS snapshot proof relies on the OS kernel and mount reporting. It is not an independent cryptographic attestation of a sealed system image or a defense against a compromised kernel/root changing the host trust basis. A system whose readonly system-volume facts cannot be established is refused with `INPUT_CHANGED`; this work does not declare a minimum macOS version from assumptions.

## Apple launcher dependency and limits

`/usr/bin/git` can be Apple's developer-tool launcher. The default OS-trusted Apple developer-tool selection remains an explicit host dependency after child selectors are removed. We do not run an unknown resolver, change `xcode-select`, install developer tools, invoke sudo or accept Homebrew. Context/proposal retain the actual canonical entry-point pin, UID/mode/device/inode/link count and bounded SHA-256. That SHA is the observed entry point's bytes, not the whole selected SDK/toolchain implementation. This is not an OS sandbox or complete toolchain attestation.

The locked rustix 1.1.5/libc 0.2.191 sources expose the Darwin descriptor APIs and public fields used here. Only offline source inspection and formatting were performed by this author. Actual Darwin compilation, real mount facts, Apple launcher behavior, Linux regressions and dual-architecture CI remain required evidence.
