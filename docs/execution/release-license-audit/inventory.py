#!/usr/bin/env python3
"""Collect the frozen lock inventory and original upstream notices without installing packages."""
import argparse
import base64
import concurrent.futures
import email
import gzip
import hashlib
import io
import json
import pathlib
import re
import subprocess
import tarfile
import tomllib
import urllib.parse
import zipfile

CARGO_LOCKS = (
    "Cargo.lock",
    "apps/desktop/src-tauri/Cargo.lock",
    "crates/remote-host/Cargo.lock",
    "tools/environment-smoke/Cargo.lock",
)
NPM_LOCKS = ("package-lock.json", "apps/desktop/package-lock.json")
PYTHON_INPUT = "requirements-infra.txt"
BROWSER_INPUT = "requirements-browser.txt"
NOTICE_NAME = re.compile(r"^(?:licen[cs]e|notice|copying|copyright|unlicense|third[-_ ]party)(?:$|[._ -])", re.I)
GRAPHS = {
    "macos_arm64_desktop": ("metadata-desktop-release.json", "agentbaton-desktop"),
    "macos_arm64_daemon": ("metadata-..json", "batond"),
    "macos_arm64_cli": ("metadata-..json", "baton"),
    "macos_arm64_remote_companion": ("metadata-crates_remote-host.json", "agentbaton-remote-host"),
    "linux_arm64_remote_companion": ("metadata-remote-aarch64-unknown-linux-gnu.json", "agentbaton-remote-host"),
    "linux_x64_remote_companion": ("metadata-remote-x86_64-unknown-linux-gnu.json", "agentbaton-remote-host"),
    "infra_smoke_tool": ("metadata-smoke.json", "agentbaton-environment-smoke"),
}


def digest(data):
    return hashlib.sha256(data).hexdigest()


def file_info(path):
    data = path.read_bytes()
    blob = hashlib.sha1(f"blob {len(data)}\0".encode() + data).hexdigest()
    return {"path": path.as_posix(), "git_blob": blob, "sha256": digest(data), "bytes": len(data)}


def read_url(url):
    parsed = urllib.parse.urlparse(url)
    if parsed.scheme != "https" or parsed.hostname not in {
        "registry.npmjs.org", "static.crates.io", "pypi.org", "files.pythonhosted.org",
    }:
        raise ValueError("unexpected registry source")
    result = subprocess.run(
        ["curl", "--proto", "=https", "--proto-redir", "=https", "-fsSL", "--connect-timeout", "8",
         "--max-time", "35", "--retry", "1", url],
        capture_output=True, check=False,
    )
    if result.returncode:
        raise OSError(f"curl exit {result.returncode}: {result.stderr.decode().strip()}")
    return result.stdout


def scan_tar(data, manifest_name, extra_paths=()):
    with tarfile.open(fileobj=io.BytesIO(data), mode="r:gz") as source:
        top = {}
        originals = {}
        manifest = None
        for item in source:
            if not item.isfile():
                continue
            parts = pathlib.PurePosixPath(item.name).parts
            if len(parts) < 2 or ".." in parts or item.name.startswith("/"):
                continue
            relative = "/".join(parts[1:])
            if relative == manifest_name:
                manifest = source.extractfile(item).read()
            if NOTICE_NAME.search(parts[-1]) or relative in extra_paths:
                originals[relative] = source.extractfile(item).read()
            if relative == ".cargo_vcs_info.json":
                top["vcs"] = json.loads(source.extractfile(item).read())
    if manifest is None:
        raise ValueError(f"missing {manifest_name} in upstream archive")
    return manifest, originals, top


def read_cargo(record, cache):
    name, version = record["name"], record["version"]
    if "source" not in record:
        return {"declaration_source": "repository_manifest", "license": None}, {}
    source = f"https://static.crates.io/crates/{name}/{name}-{version}.crate"
    cached = cache / f"{name}-{version}.crate"
    data = cached.read_bytes() if cached.is_file() else read_url(source)
    if digest(data) != record.get("checksum"):
        raise ValueError("crate archive checksum mismatch")
    manifest, originals, extra = scan_tar(data, "Cargo.toml")
    package = tomllib.loads(manifest.decode())["package"]
    if package["name"] != name or package["version"] != version:
        raise ValueError("crate identity mismatch")
    license_file = package.get("license-file")
    if license_file and license_file not in originals:
        _, originals, extra = scan_tar(data, "Cargo.toml", [license_file])
    return {
        "source": source,
        "sha256": digest(data),
        "declaration_source": "checksum_verified_crate_Cargo.toml",
        "manifest_sha256": digest(manifest),
        "license": package.get("license"),
        "license_file": license_file,
        "repository": package.get("repository"),
        "vcs": extra.get("vcs"),
    }, originals


def npm_cache_path(cache, integrity):
    algorithm, encoded = integrity.split("-", 1)
    raw = base64.b64decode(encoded, validate=True).hex()
    path = cache / "content-v2" / algorithm / raw[:2] / raw[2:4] / raw[4:]
    return path, algorithm, raw


def read_npm(key, record, cache):
    expected_name = key.rsplit("node_modules/", 1)[1]
    source, integrity = record["resolved"], record["integrity"]
    cached, algorithm, raw = npm_cache_path(cache, integrity)
    data = cached.read_bytes() if cached.is_file() else read_url(source)
    if hashlib.new(algorithm, data).hexdigest() != raw:
        raise ValueError("npm archive integrity mismatch")
    manifest, originals, _ = scan_tar(data, "package.json")
    package = json.loads(manifest)
    if package["name"] != expected_name or package["version"] != record["version"]:
        raise ValueError("npm identity mismatch")
    return {
        "source": source,
        "sha256": digest(data),
        "declaration_source": "integrity_verified_npm_package.json",
        "manifest_sha256": digest(manifest),
        "license": package.get("license"),
        "repository": package.get("repository"),
        "lock_declaration_matches": package.get("license") == record.get("license"),
    }, originals


def read_python(name, version, required_hash=None):
    source = f"https://pypi.org/pypi/{name}/{version}/json"
    release = json.loads(read_url(source))
    if required_hash:
        artifacts = [item for item in release["urls"] if item["digests"]["sha256"] == required_hash]
    else:
        wheels = [item for item in release["urls"] if item["filename"].endswith(".whl")]
        artifacts = sorted(
            wheels, key=lambda item: (
                "none-any.whl" not in item["filename"],
                not ("cp313" in item["filename"] and "macosx" in item["filename"]
                     and "arm64" in item["filename"]),
                item["filename"],
            )
        )
    chosen = artifacts[0]
    data = read_url(chosen["url"])
    if digest(data) != chosen["digests"]["sha256"]:
        raise ValueError("wheel SHA-256 mismatch")
    with zipfile.ZipFile(io.BytesIO(data)) as wheel:
        metadata_name, = [item for item in wheel.namelist() if item.endswith(".dist-info/METADATA")]
        metadata = wheel.read(metadata_name)
        patch = email.message_from_bytes(metadata)
        if patch["Version"] != version:
            raise ValueError("wheel version mismatch")
        originals = {
            item: wheel.read(item)
            for item in wheel.namelist()
            if not item.endswith("/") and (
                ".dist-info/licenses/" in item or NOTICE_NAME.search(pathlib.PurePosixPath(item).name)
            )
        }
    return {
        "source": chosen["url"],
        "release_metadata": source,
        "filename": chosen["filename"],
        "sha256": digest(data),
        "declaration_source": "digest_verified_wheel_METADATA",
        "manifest_sha256": digest(metadata),
        "license": patch.get("License-Expression") or patch.get("License"),
        "license_classifiers": [
            item for item in patch.get_all("Classifier", []) if item.startswith("License ::")
        ],
    }, originals


def resolve_profiles(runtime):
    profiles = {}
    note = {}
    for profile, (metadata, root) in GRAPHS.items():
        data = json.loads((runtime / metadata).read_text())
        packages = {}
        for package in data["packages"]:
            key = (package["name"], package["version"])
            assert key not in packages, key
            packages[key] = package["source"]
        normal = set()
        tree = runtime / f"tree-{profile}.log"
        for line in tree.read_text().splitlines():
            name, version, *_ = line.split()
            key = (name, version.removeprefix("v"))
            normal.add((*key, packages[key]))
        assert any(item[0] == root for item in normal), root
        profiles[profile] = normal
        note[profile] = {
            "root": root,
            "resolved_packages": len(data["packages"]),
            "normal_non_proc_macro_packages": len(normal),
        }
    return profiles, note


def record_originals(originals, store):
    result = []
    for path, content in sorted(originals.items()):
        identifier = digest(content)
        store[identifier] = content
        result.append({"path": path, "sha256": identifier, "bytes": len(content)})
    return result


def save_originals(output, originals):
    with (output / "OFFICIAL-NOTICES.tar.gz").open("wb") as handle:
        with gzip.GzipFile(fileobj=handle, mode="wb", filename="", mtime=0) as compressor:
            with tarfile.open(fileobj=compressor, mode="w") as archive:
                for identifier, data in sorted(originals.items()):
                    info = tarfile.TarInfo("sha256/" + identifier)
                    info.mode, info.size = 0o644, len(data)
                    archive.addfile(info, io.BytesIO(data))


def collect(args):
    output = args.output
    store = {}
    profiles, graph_counts = resolve_profiles(args.runtime)
    frozen_inputs = [file_info(pathlib.Path(name)) for name in (*CARGO_LOCKS, *NPM_LOCKS, PYTHON_INPUT)]
    records = []
    cargo = {}
    for lock in CARGO_LOCKS:
        for package in tomllib.loads(pathlib.Path(lock).read_text())["package"]:
            key = (package["name"], package["version"], package.get("source"))
            row = cargo.get(key)
            if row is None:
                row = {
                    "ecosystem": "cargo", "name": key[0], "version": key[1],
                    "lock_registry": key[2], "lock_checksum": package.get("checksum"),
                    "lock_paths": [],
                    "normal_profiles": [profile for profile in profiles if key in profiles[profile]],
                }
                details, originals = read_cargo(package, args.cargo_cache)
                row.update(details, original_notices=record_originals(originals, store))
                cargo[key] = row
                records.append(row)
            assert row["lock_checksum"] == package.get("checksum"), key
            row["lock_paths"].append(lock)

    with concurrent.futures.ThreadPoolExecutor(max_workers=6) as executor:
        pending = []
        for lock in NPM_LOCKS:
            packages = json.loads(pathlib.Path(lock).read_text())["packages"]
            for key, package in sorted(packages.items()):
                row = {
                    "ecosystem": "npm", "name": key.rsplit("node_modules/", 1)[-1] or package["name"],
                    "version": package["version"], "lock_paths": [lock], "lock_package_path": key,
                    "lock_declared_license": package.get("license"), "license": package.get("license"),
                    "lock_integrity": package.get("integrity"), "development_flag": package.get("dev", False),
                    "optional_flag": package.get("optional", False), "os": package.get("os"), "cpu": package.get("cpu"),
                    "distribution_candidate": "macos_desktop_web_assets" if key and not package.get("dev") else None,
                }
                if key:
                    pending.append((row, executor.submit(read_npm, key, package, args.npm_cache)))
                else:
                    row["declaration_source"] = "repository_package.json"
                records.append(row)
        for lock in (PYTHON_INPUT, BROWSER_INPUT):
            for line in pathlib.Path(lock).read_text().splitlines():
                if not line or line.startswith("#"):
                    continue
                prefix, *hashes = line.split()
                name, version = prefix.split("==")
                required_hash = hashes[0].removeprefix("--hash=sha256:") if hashes else None
                row = {
                    "ecosystem": "pypi", "name": name, "version": version,
                    "lock_paths": [lock], "lock_sha256": required_hash,
                    "development_flag": True, "distribution_candidate": None,
                }
                pending.append((row, executor.submit(read_python, name, version, required_hash)))
                records.append(row)
        for row, future in pending:
            try:
                details, originals = future.result()
            except (ValueError, OSError, tarfile.TarError, zipfile.BadZipFile, IndexError) as error:
                row.update(provenance="unverified", retrieval_failure={"error": type(error).__name__, "detail": str(error)})
            else:
                row.update(details, original_notices=record_originals(originals, store))

    document = {
        "source_commit": args.commit,
        "inputs": frozen_inputs,
        "supplementary_development_input": file_info(pathlib.Path(BROWSER_INPUT)),
        "cargo_resolver_profiles": graph_counts,
        "records": records,
        "unique_original_notices": len(store),
    }
    output.mkdir(parents=True, exist_ok=True)
    (output / "LOCK-INVENTORY.json").write_text(json.dumps(document, indent=2, ensure_ascii=False) + "\n")
    save_originals(output, store)
    print(f"lock inputs={len(frozen_inputs)} records={len(records)} originals={len(store)}")


def verify(path):
    output = path.parent
    data = json.loads(path.read_text())
    for item in [*data["inputs"], data["supplementary_development_input"]]:
        assert file_info(pathlib.Path(item["path"])) == item, item
    assert len(data["inputs"]) == 7
    for lock in CARGO_LOCKS:
        expected = {
            (p["name"], p["version"], p.get("source"), p.get("checksum"))
            for p in tomllib.loads(pathlib.Path(lock).read_text())["package"]
        }
        observed = {
            (r["name"], r["version"], r["lock_registry"], r["lock_checksum"])
            for r in data["records"] if lock in r["lock_paths"]
        }
        assert expected == observed, lock
    for lock in NPM_LOCKS:
        expected = {
            (key, p["version"], p.get("integrity")) for key, p in
            json.loads(pathlib.Path(lock).read_text())["packages"].items()
        }
        observed = {
            (r["lock_package_path"], r["version"], r["lock_integrity"])
            for r in data["records"] if lock in r["lock_paths"]
        }
        assert expected == observed, lock
    for lock in (PYTHON_INPUT, BROWSER_INPUT):
        expected = {
            tuple(line.split()[0].split("=="))
            for line in pathlib.Path(lock).read_text().splitlines() if line and not line.startswith("#")
        }
        observed = {(r["name"], r["version"]) for r in data["records"] if lock in r["lock_paths"]}
        assert expected == observed, lock
    ids = {item["sha256"] for row in data["records"] for item in row.get("original_notices", [])}
    assert data["unique_original_notices"] == len(ids)
    with tarfile.open(output / "OFFICIAL-NOTICES.tar.gz", "r:gz") as archive:
        for info in archive:
            assert info.isfile()
            record = info.name.removeprefix("sha256/")
            assert record != info.name and record in ids
            assert digest(archive.extractfile(info).read()) == record
            ids.remove(record)
    assert not ids, ids
    print("verified seven lock inputs, lock entry coverage, supplementary browser input and original notice hashes")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(required=True, dest="command")
    gather = commands.add_parser("collect")
    gather.add_argument("--commit", required=True)
    gather.add_argument("--runtime", type=pathlib.Path, required=True)
    gather.add_argument("--cargo-cache", type=pathlib.Path, required=True)
    gather.add_argument("--npm-cache", type=pathlib.Path, required=True)
    gather.add_argument("--output", type=pathlib.Path, required=True)
    check = commands.add_parser("verify")
    check.add_argument("inventory", type=pathlib.Path)
    args = parser.parse_args()
    if args.command == "collect":
        collect(args)
    else:
        verify(args.inventory)


if __name__ == "__main__":
    main()
