#!/usr/bin/env python3
"""Repackage the pinned ARM64 build in private storage and test native admission."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import plistlib
import shutil
import subprocess
import tempfile
import zipfile


ROOT = Path(__file__).resolve().parents[3]
PACKAGER = "scripts/package-development-macos.sh"
RECEIPT = ROOT / "docs/execution/pr33-exact-development-package/EVIDENCE.json"
COMMIT = "d70362ad1fc4b080537163be60cb830849ee4167"
APP = "AgentBaton.app"
LOCAL_APP = Path("bundle/macos") / APP
ROLE_DIR = Path("Contents/MacOS")
SEAL = "Contents/_CodeSignature/CodeResources"
MAIN = "agentbaton-desktop"

# Test fixture, invoked only on the negative replay. Every image operation
# still goes through the system hdiutil. Modify one sealed resource after the
# product has verified and pinned staging, immediately before image creation.
TAMPER_HDIUTIL = """#!/usr/bin/env python3
import os
from pathlib import Path
import sys

if sys.argv[1] == "create":
    stage = Path(sys.argv[sys.argv.index("-srcfolder") + 1])
    expected = Path(os.environ["AB_ADMISSION_FIXTURE_REPOSITORY"]) / "artifacts"
    assert stage.name == "stage" and stage.parent.parent == expected
    assert stage.parent.name.startswith(".development.")
    resource = stage / "AgentBaton.app/Contents/Resources/icon.icns"
    with resource.open("ab") as handle:
        handle.write(b"AB-REL-004 private sealed resource mutation")
    print("CONTROL: changed a sealed icon before real hdiutil create", file=sys.stderr, flush=True)
os.execv("/usr/bin/hdiutil", ["/usr/bin/hdiutil", *sys.argv[1:]])
"""


def require(condition, reason):
    if not condition:
        raise RuntimeError(reason)


def digest(path):
    with path.open("rb") as handle:
        return hashlib.file_digest(handle, "sha256").hexdigest()


def execute(*argv, cwd=ROOT, env=None, timeout=180):
    return subprocess.run(argv, cwd=cwd, env=env, timeout=timeout,
                          capture_output=True, text=True, errors="replace", check=False)


def checked(result):
    require(result.returncode == 0, f"{Path(result.args[0]).name} exit {result.returncode}")
    return result.stdout


def git(cwd, *args):
    return checked(execute("git", *args, cwd=cwd)).strip()


def hash_build(bare, app):
    files = {f"compiled/{name}": digest(path) for name, path in bare.items()}
    for path in sorted(app.rglob("*")):
        require(not path.is_symlink(), "Unexpected symlink in fixed build outputs")
        if path.is_file():
            files[f"assembled/{path.relative_to(app)}"] = digest(path)
    return files


def snapshot(deliverables, tracked):
    digests = dict(tracked["artifact"]["archive_member_sha256"])
    digests.update({"development.zip": tracked["artifact"]["archive_sha256"],
                    "provenance.json": tracked["logs_sha256"]["provenance.json"]})
    require({name: digest(deliverables / name) for name in digests} == digests,
            "Fixed deliverable bytes differ from the recorded build")
    return digests


def sanitized(result, private, original, deliverables):
    replacements = [(str(original), "<FIXED_BUILD>"), (str(deliverables), "<FIXED_DELIVERABLES>"),
                    (str(ROOT), "<TASK_WORKTREE>"), (str(private), "<TASK_PRIVATE>"),
                    (str(Path.home()), "<HOME>")]

    def redact(text):
        for value, replacement in replacements:
            text = text.replace(value, replacement)
        return text

    return {"exit": result.returncode, "stdout": redact(result.stdout), "stderr": redact(result.stderr)}


def owned_mounts(private):
    result = execute("/usr/bin/hdiutil", "info", "-plist")
    rows = plistlib.loads(checked(result).encode())["images"]
    return [row["image-path"] for row in rows if row.get("image-path", "").startswith(f"{private}/")]


def assert_clean(source, private):
    require(not (source / "artifacts/development").exists(), "Unexpected output after refusal")
    require(not list((source / "artifacts").glob(".development.*")), "Staging directory was left behind")
    require(not owned_mounts(private), "Task-owned image was left attached")


def repackage(private, deliverables, report):
    tracked = json.loads(RECEIPT.read_bytes())
    build = tracked["build"]
    require(build["source_commit"] == COMMIT and git(ROOT, "rev-parse", f"{COMMIT}^{{tree}}")
            == build["source_tree"], "Fixed source tree differs")
    original = Path(build["checkout"])
    app = Path(build["application"])
    roles = build["compiled_originals_sha256"]
    bare = {name: original / "target/release" / name for name in roles}
    bare[MAIN] = app.parents[2] / MAIN
    before = hash_build(bare, app)
    require(all(before[f"compiled/{role}"] == before[f"assembled/{ROLE_DIR / role}"] == sha
                for role, sha in roles.items()), "Original binaries differ from fixed receipt")
    report["input"] = {"commit": COMMIT, "tree": build["source_tree"], "files": before,
                       "receipt_sha256": digest(RECEIPT), "deliverables": snapshot(deliverables, tracked)}
    report["tool_paths"] = {tool: shutil.which(tool) for tool in ("codesign", "hdiutil")}
    require(report["tool_paths"] == {"codesign": "/usr/bin/codesign", "hdiutil": "/usr/bin/hdiutil"},
            "System macOS tools required")

    source = private / "source"
    output = source / "artifacts/development"
    target = private / "target"
    release = target / "release"
    release.mkdir(parents=True)
    private_app = release / LOCAL_APP
    private_app.parent.mkdir(parents=True)
    repo = Path(git(ROOT, "rev-parse", "--path-format=absolute", "--git-common-dir")).parent
    try:
        checked(execute("git", "clone", "--shared", "--no-checkout", "--quiet", repo, source))
        git(source, "checkout", "--quiet", "--detach", COMMIT)
        require(git(source, "status", "--porcelain") == "", "Private checkout is not pristine")
        shutil.copy2(ROOT / PACKAGER, source / PACKAGER)
        require(git(source, "diff", "--name-only") == PACKAGER, "Unexpected private checkout overlay")
        for role, path in bare.items():
            shutil.copy2(path, release / role)
        checked(execute("/usr/bin/ditto", app, private_app))
        original_copy = hash_build({role: release / role for role in roles}, private_app)
        require(original_copy == before, "Private build copy differs")
        env = dict(os.environ, CARGO_TARGET_DIR=str(target), CARGO_NET_OFFLINE="true", TMPDIR=str(private))

        def package(label, **overrides):
            result = execute("bash", PACKAGER, cwd=source, env=dict(env, **overrides))
            report[label] = sanitized(result, private, original, deliverables)
            return result

        checked(package("valid_native_package"))
        require(not owned_mounts(private), "Positive packager left a mount")
        shipped = private / "positive"
        output.rename(shipped)
        require(len(list(shipped.iterdir())) == 4, "Unexpected package output")
        record = (shipped / "build-record.txt").read_text()
        fields = dict(line.split("=", 1) for line in record.splitlines() if "=" in line)
        require(fields["commit"] == COMMIT and fields["signing_mode"] == "ad_hoc_app"
                and fields["Signature"] == "adhoc" and "Executable" not in fields,
                "Build record did not bind the ad hoc package")
        dmg = shipped / Path(tracked["artifact"]["dmg_path"]).name
        require(digest(dmg) == fields["dmg_sha256"]
                and (shipped / "SHA256SUMS").read_text() == f"{digest(dmg)}  ./{dmg.name}\n",
                "Image SHA-256 did not match both records")
        archive = private / "development.zip"
        with zipfile.ZipFile(archive, "x", compression=zipfile.ZIP_DEFLATED) as zipped:
            for path in sorted(shipped.iterdir()):
                zipped.write(path, path.name)
        verified = execute("python3", ROOT / "scripts/record-development-bundle-provenance.py",
                           "--archive", archive, "--expected-archive-sha256", digest(archive),
                           "--expected-commit", COMMIT, env=env)
        report["native_provenance"] = sanitized(verified, private, original, deliverables)
        receipt = json.loads(checked(verified))
        files = {row["path"]: row for row in receipt["files"]}
        require(receipt["identity"]["dmg_sha256"] == fields["dmg_sha256"]
                and files[f"{APP}/{SEAL}"]["sha256"] == fields["code_resources_sha256"],
                "Read-only provenance differs from the build record")
        for role, compile_sha in roles.items():
            shipped_row = files[f"{APP}/{ROLE_DIR / role}"]
            require(fields[f"compiled_sha256.{role}"] == compile_sha
                    and fields[f"shipped_sha256.{role}"] == shipped_row["sha256"]
                    and shipped_row["signature"]["verification"] == "passed",
                    f"Read-only provenance mismatch for {role}")
            if role != MAIN:
                require(shipped_row["sha256"] == compile_sha, f"Helper bytes changed: {role}")
        require(files[f"{APP}/{ROLE_DIR / MAIN}"]["signature"]["type"] == "ad_hoc"
                and fields[f"shipped_sha256.{MAIN}"] != roles[MAIN], "Desktop transform not observed")
        report["artifact"] = {
            "files_sha256": {path.name: digest(path) for path in sorted(shipped.iterdir())},
            "zip_sha256": digest(archive),
            "compiled_sha256": roles,
            "shipped_sha256": {role: fields[f"shipped_sha256.{role}"] for role in roles},
            "seal_sha256": fields["code_resources_sha256"],
            "main_signature": files[f"{APP}/{ROLE_DIR / MAIN}"]["signature"],
        }
        assert_clean(source, private)
        report["accepted_package_removed_from_output_for_controls"] = True

        staged_helper = private_app / ROLE_DIR / "batond"
        try:
            with staged_helper.open("ab") as handle:
                handle.write(b"AB-REL-004 private compiled pin control")
            rejected = package("refused_compiled_pin_change")
            require(rejected.returncode != 0 and "SHA-256 mismatch for batond" in rejected.stderr,
                    "Real staging admitted a changed compiled pin")
        finally:
            shutil.copy2(app / ROLE_DIR / "batond", staged_helper)
        assert_clean(source, private)
        report["compiled_pin_rejection_clean"] = True

        command_dir = private / "negative-tools"
        command_dir.mkdir()
        tamper = command_dir / "hdiutil"
        tamper.write_text(TAMPER_HDIUTIL)
        tamper.chmod(0o700)
        rejected = package("refused_mounted_sealed_resource_change",
                           PATH=f"{command_dir}{os.pathsep}{env['PATH']}",
                           AB_ADMISSION_FIXTURE_REPOSITORY=str(source))
        assert_clean(source, private)
        report["mounted_resource_rejection_clean"] = True
        require(rejected.returncode == 1
                and "CONTROL: changed a sealed icon before real hdiutil create" in rejected.stderr
                and "/volume/AgentBaton.app: a sealed resource is missing or invalid" in rejected.stderr
                and rejected.stdout.count("PASS no inspected native harness identifiers") == 2,
                "Real mounted verification missed an icon mutation")
        report["build_copy_unchanged"] = hash_build({role: release / role for role in roles}, private_app) == before
        require(report["build_copy_unchanged"], "Packager changed private compiler or assembled originals")
    finally:
        report["fixed_originals_unchanged"] = (hash_build(bare, app) == before
                                                and snapshot(deliverables, tracked) == report["input"]["deliverables"])
        report["owned_mounts_after_replay"] = len(owned_mounts(private))
        require(report["fixed_originals_unchanged"] and not report["owned_mounts_after_replay"],
                "Originals changed or task mount leaked")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--deliverables", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    require(platform.system() == "Darwin" and platform.machine() == "arm64", "Apple Silicon required")
    deliverables = args.deliverables.resolve(strict=True)
    private = Path(tempfile.mkdtemp(prefix="native-adhoc-admission-",
                                    dir=Path(os.environ["BATON_RUNTIME_DIR"]).resolve(strict=True)))
    print(f"Private replay directory: {private}", flush=True)
    with args.output.open("x") as handle:
        report = {"task": "AB-REL-004", "status": "running", "architecture": platform.machine(),
                  "macos": checked(execute("/usr/bin/sw_vers")).strip(),
                  "xcode": checked(execute("/usr/bin/xcodebuild", "-version")).strip(),
                  "execution_head": git(ROOT, "rev-parse", "HEAD"),
                  "worktree_status": git(ROOT, "status", "--porcelain").splitlines(),
                  "packager_sha256": digest(ROOT / PACKAGER), "replay_sha256": digest(Path(__file__))}
        try:
            repackage(private, deliverables, report)
            report["status"] = "passed"
        except Exception as error:
            report.update(status="failed", error=str(error).replace(str(private), "<TASK_PRIVATE>"))
            raise
        finally:
            json.dump(report, handle, indent=2)
            handle.write("\n")


if __name__ == "__main__":
    main()
