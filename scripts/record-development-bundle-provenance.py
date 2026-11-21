#!/usr/bin/env python3
"""Record provenance for a pinned AgentBaton ARM64 development archive."""
import argparse
import hashlib
import json
import os
import pathlib
import plistlib
import re
import shutil
import stat
import struct
import subprocess
import sys
import tempfile
import zipfile
from contextlib import contextmanager

ROOT = pathlib.Path(__file__).resolve().parents[1]
TARGET = "aarch64-apple-darwin"
SHA256 = re.compile(r"[0-9a-f]{64}")
GIT_SHA = re.compile(r"[0-9a-f]{40}")
ARTIFACT_LIMIT = 512 * 1024 * 1024
RECEIPT_LIMIT = 32 * 1024 * 1024
CHECKLIST = "DEVELOPMENT_ACCEPTANCE.md"
BUILD_RECORD = "build-record.txt"
CHECKSUMS = "SHA256SUMS"
TTY_ENV = {"PATH": "/usr/bin:/bin", "LANG": "C", "LC_ALL": "C"}
MACH_MAGIC = {b"\xcf\xfa\xed\xfe", b"\xfe\xed\xfa\xcf", b"\xca\xfe\xba\xbe",
              b"\xca\xfe\xba\xbf", b"\xbe\xba\xfe\xca", b"\xbf\xba\xfe\xca",
              b"\xce\xfa\xed\xfe", b"\xfe\xed\xfa\xce"}


class EvidenceError(ValueError):
    pass


def require(condition, message):
    if not condition:
        raise EvidenceError(message)


def digest(data):
    return hashlib.sha256(data).hexdigest()


def file_digest(path):
    with path.open("rb") as handle:
        return hashlib.file_digest(handle, "sha256").hexdigest()


def safe_file(path):
    require(not path.is_symlink() and path.is_file(), "expected a regular input file")
    return path


def capture(source, destination, maximum):
    require(safe_file(source).stat().st_size <= maximum, "input exceeds size limit")
    with source.open("rb") as reader, destination.open("xb") as writer:
        remaining = maximum
        while block := reader.read(min(remaining + 1, 1024 * 1024)):
            require(len(block) <= remaining, "input exceeds size limit")
            writer.write(block)
            remaining -= len(block)
    return file_digest(destination)


def unique_json(data):
    def pairs(items):
        result = dict(items)
        require(len(items) == len(result), "duplicate JSON field")
        return result

    return json.loads(data, object_pairs_hook=pairs)


def git_bytes(repository, *args):
    command = subprocess.run(
        ["git", "--no-replace-objects", "-C", str(repository), *args],
        capture_output=True, timeout=20,
    )
    require(command.returncode == 0, "Git build input could not be verified")
    return command.stdout


def source_inputs(repository, commit):
    require(git_bytes(repository, "cat-file", "-t", commit).strip() == b"commit",
            "expected a Git commit")
    tree = git_bytes(repository, "rev-parse", f"{commit}^{{tree}}").decode().strip()
    config_path = "apps/desktop/src-tauri/tauri.conf.json"
    raw = git_bytes(repository, "show", f"{commit}:{config_path}")
    config = unique_json(raw)
    require(config["productName"] == "AgentBaton" and config["identifier"] == "dev.agentbaton",
            "unexpected source bundle identity")
    sidecars = config["bundle"]["externalBin"]
    require(sidecars == ["binaries/batond", "binaries/baton"], "unexpected source sidecar list")
    known = {
        "AgentBaton.app/Contents/Resources/icon.icns": "apps/desktop/src-tauri/icons/icon.icns",
    }
    return {
        "commit": commit,
        "tree": tree,
        "version": config["version"],
        "identifier": config["identifier"],
        "config_sha256": digest(raw),
        "checklist_sha256": digest(git_bytes(repository, "show", f"{commit}:docs/execution/{CHECKLIST}")),
        "repo_resources": {
            output: {"path": source, "sha256": digest(git_bytes(repository, "show", f"{commit}:{source}"))}
            for output, source in known.items()
        },
        "executables": {
            "AgentBaton.app/Contents/MacOS/agentbaton-desktop": "desktop",
            "AgentBaton.app/Contents/MacOS/batond": "daemon",
            "AgentBaton.app/Contents/MacOS/baton": "cli",
        },
    }


def checked_git_sha(value):
    require(isinstance(value, str) and GIT_SHA.fullmatch(value), "invalid Git commit")
    return value


def checked_digest(value):
    require(isinstance(value, str) and SHA256.fullmatch(value), "invalid SHA-256")
    return value


def read_zip_member(archive, name, size):
    item = archive.getinfo(name)
    require(item.file_size <= size and (item.external_attr >> 16 & 0o170000) in (0, stat.S_IFREG),
            "unsupported archive member")
    return archive.read(item)


def unpack_archive(snapshot, private, inputs):
    commit = inputs["commit"]
    name = f"AgentBaton-arm64-{commit}-development.dmg"
    with zipfile.ZipFile(snapshot) as archive:
        names = archive.namelist()
        require(len(names) == 4 and set(names) == {name, BUILD_RECORD, CHECKSUMS, CHECKLIST},
                "unexpected or duplicate development archive entries")
        record = read_zip_member(archive, BUILD_RECORD, 65536)
        text = record.decode()
        expected_metadata = {
            "commit": commit,
            "architecture": "arm64",
            "channel": "development; no Developer ID signing or notarization configured",
        }
        for key, expected in expected_metadata.items():
            require(re.findall(rf"^{key}=(.*)$", text, re.MULTILINE) == [expected],
                    f"build record {key} mismatch")
        checksum = read_zip_member(archive, CHECKSUMS, 1024).decode()
        match = re.fullmatch(r"([0-9a-f]{64})  (?:\./)?" + re.escape(name) + r"\n?", checksum)
        require(match is not None, "invalid development DMG checksum manifest")
        checklist = read_zip_member(archive, CHECKLIST, 1024 * 1024)
        require(digest(checklist) == inputs["checklist_sha256"], "acceptance checklist does not match build commit")
        item = archive.getinfo(name)
        require(0 < item.file_size <= ARTIFACT_LIMIT
                and (item.external_attr >> 16 & 0o170000) in (0, stat.S_IFREG), "unsupported DMG archive member")
        dmg = private / name
        with archive.open(item) as source, dmg.open("xb") as destination:
            shutil.copyfileobj(source, destination, 1024 * 1024)
    require(file_digest(dmg) == match[1], "development DMG SHA-256 mismatch")
    return dmg, {
        "name": name, "sha256": match[1], "build_record_sha256": digest(record),
        "checklist_sha256": digest(checklist), "checksum_manifest_sha256": digest(checksum.encode()),
        "channel": "development",
    }


@contextmanager
def mounted_dmg(dmg, directory):
    require(sys.platform == "darwin" and os.uname().machine == "arm64",
            "read-only development DMG inspection requires native macOS ARM64")
    directory.mkdir()
    attached = False
    try:
        command = subprocess.run(
            ["/usr/bin/hdiutil", "attach", "-readonly", "-noautoopen", "-nobrowse", "-mountpoint",
             str(directory), str(dmg)], capture_output=True, timeout=60, env=TTY_ENV,
        )
        attached = command.returncode == 0
        require(attached, "read-only DMG attach failed")
        yield directory
    finally:
        if attached or os.path.ismount(directory):
            detached = subprocess.run(
                ["/usr/bin/hdiutil", "detach", str(directory)], capture_output=True, timeout=45, env=TTY_ENV,
            )
            require(detached.returncode == 0, "read-only DMG detach failed")


def inspect_macho(path):
    with path.open("rb") as source:
        header = source.read(32)
        require(len(header) == 32, "truncated Mach-O header")
        magic, cpu, subtype, kind, count, size, _, _ = struct.unpack("<8I", header)
        require(magic == 0xFEEDFACF and cpu == 0x0100000C and kind == 2,
                "expected a thin ARM64 Mach-O executable")
        require(0 < count <= size // 8 and size <= min(path.stat().st_size - 32, 4 * 1024 * 1024),
                "invalid Mach-O load-command size")
        commands = source.read(size)
    require(len(commands) == size, "truncated Mach-O load commands")
    position = 0
    for _ in range(count):
        require(position + 8 <= size, "truncated Mach-O load command")
        _, command_size = struct.unpack_from("<2I", commands, position)
        require(8 <= command_size <= size - position and command_size % 8 == 0, "invalid Mach-O load command")
        position += command_size
    require(position == size, "unexpected Mach-O command data")
    return {"architecture": "arm64", "cpu_subtype": subtype, "binary_format": "Mach-O 64-bit thin"}


def inspect_signature(path):
    command = subprocess.run(
        ["/usr/bin/codesign", "--display", "--verbose=4", str(path)],
        capture_output=True, timeout=20, env=TTY_ENV,
    )
    text = command.stderr.decode(errors="replace")
    if command.returncode and "code object is not signed at all" in text:
        return {"type": "unsigned", "verification": "not_applicable", "publisher_trust": "unknown"}
    if command.returncode:
        return {"type": "unknown", "verification": "unknown", "publisher_trust": "unknown"}
    adhoc = re.search(r"^Signature=adhoc$", text, re.MULTILINE) is not None
    developer_id = re.search(r"^Authority=Developer ID Application:", text, re.MULTILINE) is not None
    signature_type = "ad_hoc" if adhoc else "developer_id_observed" if developer_id else "other_signature_observed"
    identifier = re.search(r"^CDHash=([0-9a-f]{40})$", text, re.MULTILINE)
    team_id = re.search(r"^TeamIdentifier=([A-Z0-9]{10})$", text, re.MULTILINE)
    verified = subprocess.run(
        ["/usr/bin/codesign", "--verify", "--strict", str(path)],
        capture_output=True, timeout=25, env=TTY_ENV,
    )
    return {
        "type": signature_type, "cdhash": identifier[1] if identifier else None,
        "team_identifier": team_id[1] if team_id else None,
        "verification": "passed" if verified.returncode == 0 else "failed",
        "verification_exit_code": verified.returncode,
        "publisher_trust": "unknown",
    }


def inspect_volume(volume, inputs):
    app = volume / "AgentBaton.app"
    require(app.is_dir() and not app.is_symlink(), "expected AgentBaton.app")
    link = volume / "Applications"
    require(link.is_symlink() and os.readlink(link) == "/Applications", "invalid Applications shortcut")
    require(all(path.name in {"AgentBaton.app", "Applications", ".DS_Store", ".fseventsd", ".Trashes"}
                for path in volume.iterdir()), "unexpected development volume item")
    def on_walk_error(error):
        raise EvidenceError("bundle directory could not be enumerated") from error

    rows = {}
    for here, directories, names in os.walk(app, onerror=on_walk_error, followlinks=False):
        root = pathlib.Path(here)
        for directory in directories:
            require(not (root / directory).is_symlink(), "symlink inside app bundle")
        for name in names:
            path = safe_file(root / name)
            relative = path.relative_to(volume).as_posix()
            row = {
                "path": relative, "sha256": file_digest(path), "bytes": path.stat().st_size,
                "role": "resource", "attribution": [],
            }
            executable = inputs["executables"].get(relative)
            if executable:
                row.update(role=executable, **inspect_macho(path), signature=inspect_signature(path))
            else:
                with path.open("rb") as handle:
                    require(handle.read(4) not in MACH_MAGIC, "unrecognized executable in development bundle")
            source = inputs["repo_resources"].get(relative)
            if source and row["sha256"] == source["sha256"]:
                row["attribution"].append({"kind": "git_source_bytes", **source})
            rows[relative] = row
    require(inputs["executables"].keys() <= rows.keys(), "missing AgentBaton executable")
    info_path = "AgentBaton.app/Contents/Info.plist"
    require(info_path in rows and rows[info_path]["bytes"] <= 65536, "missing or oversized bundle plist")
    plist = plistlib.loads((volume / info_path).read_bytes())
    require(isinstance(plist, dict) and plist.get("CFBundleIdentifier") == inputs["identifier"]
            and plist.get("CFBundleShortVersionString") == inputs["version"]
            and plist.get("CFBundleVersion") == inputs["version"]
            and plist.get("CFBundleExecutable") == "agentbaton-desktop", "bundle plist mismatch with source")
    rows[info_path]["role"] = "bundle_metadata"
    return rows


def local_receipt_file(directory, name, sha, maximum=RECEIPT_LIMIT):
    require(isinstance(name, str) and re.fullmatch(r"[A-Za-z0-9_-][A-Za-z0-9_.-]*", name),
            "invalid receipt sidecar filename")
    path = safe_file(directory / name)
    require(path.stat().st_size <= maximum, "receipt sidecar exceeds size limit")
    content = path.read_bytes()
    require(digest(content) == checked_digest(sha), "receipt sidecar SHA-256 mismatch")
    return content


def exact_fields(data, names, message):
    require(isinstance(data, dict) and set(data) == set(names), message)


def parse_link_map(data, output):
    lines = data.decode().splitlines()
    require(len(lines) >= 6 and lines[0].startswith("# Path: ") and lines[1] == "# Arch: arm64",
            "expected Apple ARM64 linker map")
    path = pathlib.PurePosixPath(lines[0][8:])
    require(path.name == output, "link map output name mismatch")
    require(lines[2] == "# Object files:" and "# Sections:" in lines and "# Symbols:" in lines,
            "incomplete linker map")
    sections, symbols = lines.index("# Sections:"), lines.index("# Symbols:")
    require(3 < sections < symbols < len(lines) - 1, "missing linker map records")
    objects = []
    ids = set()
    for line in lines[3:sections]:
        entry = re.fullmatch(r"\[\s*(\d+)\]\s+(.+)", line)
        require(entry is not None, "invalid link map object")
        index = int(entry[1])
        require(index not in ids, "duplicate link map object")
        ids.add(index)
        if index:
            objects.append(pathlib.PurePosixPath(entry[2]).name)
    require(objects, "link map has no inputs")
    found_symbol = False
    for line in lines[symbols + 1:]:
        if line.startswith("# Dead Stripped Symbols:"):
            break
        if line.startswith("#"):
            continue
        match = re.fullmatch(r"0x[0-9a-fA-F]+\s+0x[0-9a-fA-F]+\s+\[\s*(\d+)\]\s+\S.*", line)
        require(match is not None and int(match[1]) in ids, "invalid linker map symbol")
        found_symbol |= int(match[1]) != 0
    require(found_symbol, "link map has no observed input symbol")
    return objects


def linker_attribution(binding, directory, row):
    exact_fields(binding, {"kind", "file", "file_sha256", "record", "record_sha256", "linked_output"},
                 "invalid linker receipt binding")
    require("architecture" in row, "linker receipt file is not a verified Mach-O")
    output = binding["linked_output"]
    local_receipt_file(directory, output, row["sha256"], ARTIFACT_LIMIT)
    record = local_receipt_file(directory, binding["record"], binding["record_sha256"])
    objects = parse_link_map(record, output)
    return {"kind": "linker_map_receipt", "sha256": binding["record_sha256"], "object_inputs": objects}


def web_attribution(binding, directory, volume, row):
    exact_fields(
        binding, {"kind", "file", "file_sha256", "record", "record_sha256",
                  "asset", "asset_sha256", "offset"}, "invalid Web receipt binding",
    )
    asset = local_receipt_file(directory, binding["asset"], binding["asset_sha256"])
    offset = binding["offset"]
    require(asset and type(offset) is int and 0 <= offset <= row["bytes"] - len(asset),
            "Web asset offset outside bundle file")
    require("architecture" in row or (offset == 0 and len(asset) == row["bytes"]),
            "standalone Web resource must match the full file")
    with (volume / row["path"]).open("rb") as carrier:
        carrier.seek(offset)
        require(carrier.read(len(asset)) == asset, "Web asset bytes do not match bundle file")
    record = unique_json(local_receipt_file(directory, binding["record"], binding["record_sha256"]))
    exact_fields(record, {"schema_version", "chunk", "chunk_sha256", "modules"}, "invalid Web module record")
    require(type(record["schema_version"]) is int and record["schema_version"] == 1
            and record["chunk"] == binding["asset"] and record["chunk_sha256"] == binding["asset_sha256"],
            "Web record asset mismatch")
    modules = record["modules"]
    require(isinstance(modules, list) and modules, "empty Web module record")
    names = set()
    for item in modules:
        exact_fields(item, {"id", "rendered_length"}, "invalid Web module")
        identifier, length = item["id"], item["rendered_length"]
        require(isinstance(identifier, str) and re.fullmatch(r"[A-Za-z0-9_@.+:/-]{1,1024}", identifier)
                and not identifier.startswith("/") and ".." not in identifier.split("/")
                and identifier not in names and type(length) is int and length >= 0, "invalid Web module")
        names.add(identifier)
    require(any(item["rendered_length"] > 0 for item in modules), "no rendered Web modules")
    return {
        "kind": "web_module_receipt", "sha256": binding["record_sha256"], "asset_sha256": binding["asset_sha256"],
        "asset_bytes": len(asset), "offset": offset, "modules": modules,
    }


def apply_receipt(path, pinned, identity, rows, volume):
    data = local_receipt_file(path.parent, path.name, pinned)
    receipt = unique_json(data)
    exact_fields(receipt, {"schema_version", "archive_sha256", "dmg_sha256", "source_commit",
                           "source_tree", "target", "bindings"}, "invalid development receipt")
    require(type(receipt["schema_version"]) is int and receipt["schema_version"] == 1,
            "unsupported development receipt schema")
    require(all(receipt[key] == value for key, value in identity.items()), "receipt package identity mismatch")
    bindings = receipt["bindings"]
    require(isinstance(bindings, list) and bindings, "empty development receipt")
    seen = set()
    for binding in bindings:
        require(isinstance(binding, dict), "invalid receipt binding")
        file, kind = binding.get("file"), binding.get("kind")
        require(isinstance(file, str) and file in rows and kind in ("linker", "web"),
                "unknown receipt kind or bundle file")
        row = rows[file]
        require(binding.get("file_sha256") == row["sha256"], "receipt bundle file SHA-256 mismatch")
        require((file, kind) not in seen, "duplicate receipt file attribution")
        seen.add((file, kind))
        evidence = (linker_attribution(binding, path.parent, row) if kind == "linker"
                    else web_attribution(binding, path.parent, volume, row))
        row["attribution"].append(evidence)


def report(args):
    commit = checked_git_sha(args.expected_commit)
    expected_archive = checked_digest(args.expected_archive_sha256)
    require(bool(args.receipt) == bool(args.receipt_sha256), "receipt and externally pinned SHA-256 must be paired")
    confirmed = source_inputs(args.repo, commit)
    with tempfile.TemporaryDirectory(prefix="ab-bundle-") as temp:
        private = pathlib.Path(temp)
        snapshot = private / "development.zip"
        require(capture(args.archive, snapshot, ARTIFACT_LIMIT) == expected_archive, "archive SHA-256 mismatch")
        dmg, artifact = unpack_archive(snapshot, private, confirmed)
        identity = {
            "archive_sha256": expected_archive,
            "dmg_sha256": artifact["sha256"],
            "source_commit": commit,
            "source_tree": confirmed["tree"],
            "target": TARGET,
        }
        with mounted_dmg(dmg, private / "volume") as volume:
            files = inspect_volume(volume, confirmed)
            if args.receipt:
                apply_receipt(args.receipt, args.receipt_sha256, identity, files, volume)
    for row in files.values():
        row["attribution_state"] = "bound_evidence" if row["attribution"] else "unknown"
        if row.get("architecture"):
            kinds = {item["kind"] for item in row["attribution"]}
            row["static_linkage"] = "bound_receipt" if "linker_map_receipt" in kinds else "unknown"
            if row["role"] == "desktop":
                row["embedded_web"] = "bound_receipt" if "web_module_receipt" in kinds else "unknown"
    return {
        "schema_version": 1,
        "identity": identity,
        "artifact": artifact,
        "product_version": confirmed["version"],
        "source_config_sha256": confirmed["config_sha256"],
        "inspection": "macos_arm64_read_only_dmg_detached",
        "mount_shortcut": {"path": "Applications", "target": "/Applications"},
        "notarization": "unknown",
        "test_execution": "unknown",
        "receipt_sha256": args.receipt_sha256,
        "receipt_trust_anchor": "externally_pinned_trusted_build_record_required",
        "attribution_completeness": "unknown",
        "files": [files[key] for key in sorted(files)],
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--archive", type=pathlib.Path, required=True)
    parser.add_argument("--expected-archive-sha256", required=True)
    parser.add_argument("--expected-commit", required=True)
    parser.add_argument("--repo", type=pathlib.Path, default=ROOT)
    parser.add_argument("--receipt", type=pathlib.Path)
    parser.add_argument("--receipt-sha256", help="expected digest from a trusted, separate build record")
    args = parser.parse_args()
    try:
        output = report(args)
    except (ValueError, OSError, KeyError, TypeError, zipfile.BadZipFile, plistlib.InvalidFileException,
            subprocess.TimeoutExpired) as error:
        message = str(error) if isinstance(error, EvidenceError) else "invalid or unavailable provenance input"
        print(f"development bundle provenance: {message}", file=sys.stderr)
        return 1
    print(json.dumps(output, indent=2))
    return 0


if __name__ == "__main__":
    sys.exit(main())
