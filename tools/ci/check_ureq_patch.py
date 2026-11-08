#!/usr/bin/env python3
"""Verify Borrowser's exact cumulative ureq 2.12.1 fork and selected features."""

import argparse
import hashlib
import json
from pathlib import Path
import shutil
import stat
import subprocess
import sys
import tarfile
import tempfile
import tomllib

ARCHIVE_SHA256 = "02d1a66277ed75f640d608235660df48c8e3c19f3b4edb6a263315626cc3c01d"
UPSTREAM_GIT = "ba4555d54fd72e16fc737f7e8fcc812efb131260"
UPSTREAM_FILES_SHA256 = "6c820757679ccd8200e73c04286f3d77d8b1e7af3af3c6c98c36a4f8f6932da3"
AI3_PATCH_SHA256 = "0c2eb5cda5ecf67a476457de2dffefbdb82c910f7c8f49b1d07ba874dfd9d969"
AI3_FILES_SHA256 = "2c75dfc6b1e10a27fa2458db2a80e2e87e032eaaa8d376c0e78322a4f1ab5ef3"
PATCH_SHA256 = "3ad93bedc381f0ce6df55b87c9f31c67b23b1b98775576c87c24b1c8450e6138"
PATCHED_FILES_SHA256 = "5a60ed70bde22c4898f6a8cf339acaddb3c40ddc939501de27568e9707bee50b"
SOURCE_FILES = {
    "Cargo.toml", "Cargo.toml.orig", "src/chunked/decoder.rs", "src/error.rs",
    "src/header.rs", "src/pool.rs", "src/response.rs", "src/rtls.rs",
    "src/stream.rs", "src/unit.rs",
}
PATCH_FILE = "ai5-response-integrity.patch"
RECORD_FILES = {"UPSTREAM.json", "BORROWSER-PATCH.md", "response-header.patch", PATCH_FILE}
OMITTED_FILES = {".github/dependabot.yml", ".github/workflows/test.yml", ".gitignore", "Cargo.lock"}
# Names from the already frozen published manifest, excluding the four omissions.
# Keep this independent of UPSTREAM.json so inventory admission needs no record
# or source reads. Its agreement with the pinned manifest is checked below.
RETAINED_FILES = SOURCE_FILES | {
    ".cargo_vcs_info.json", "CHANGELOG.md", "CONTRIBUTING.md", "FUTURE.md",
    "LICENSE-APACHE", "LICENSE-MIT", "README.md", "README.tpl", "RELEASE.txt",
    "examples/count-bytes.rs", "examples/cureq/main.rs", "examples/custom-tls.rs",
    "examples/ipv6.rs", "examples/smoke-test/main.rs", "examples/tls_config.rs",
    "src/agent.rs", "src/body.rs", "src/chunked/LICENSE-APACHE",
    "src/chunked/LICENSE-MIT", "src/chunked/mod.rs", "src/cookies.rs",
    "src/http_crate.rs", "src/http_interop.rs", "src/lib.rs", "src/middleware.rs",
    "src/ntls.rs", "src/proxy.rs", "src/request.rs", "src/resolve.rs",
    "src/test/agent_test.rs", "src/test/body_read.rs", "src/test/body_send.rs",
    "src/test/mod.rs", "src/test/query_string.rs", "src/test/range.rs",
    "src/test/redirect.rs", "src/test/simple.rs", "src/test/timeout.rs",
    "src/testserver.rs", "tests/https-agent.rs",
}


def digest(data):
    return hashlib.sha256(data).hexdigest()


def manifest_digest(files):
    return digest(json.dumps(files, sort_keys=True, separators=(",", ":")).encode())


def require_real_directory(directory):
    mode = directory.lstat().st_mode
    if stat.S_ISLNK(mode):
        raise ValueError("ureq inventory contains a symlink")
    if not stat.S_ISDIR(mode):
        raise ValueError("ureq inventory requires a real directory")


def checked_files(directory, expected):
    """Admit exact names and entry types without opening any file contents."""
    require_real_directory(directory)
    directories = {parent.as_posix() for name in expected
                   for parent in Path(name).parents if parent != Path(".")}
    files = {}
    for path in directory.rglob("*"):
        name = path.relative_to(directory).as_posix()
        mode = path.lstat().st_mode
        if stat.S_ISLNK(mode):
            raise ValueError("ureq inventory contains a symlink")
        if stat.S_ISDIR(mode):
            if name not in directories:
                raise ValueError("ureq vendored file inventory changed")
            continue
        if not stat.S_ISREG(mode):
            raise ValueError("ureq inventory contains an unsupported entry")
        if name not in expected:
            raise ValueError("ureq vendored file inventory changed")
        if name in files:
            raise ValueError("ureq inventory contains a duplicate entry")
        files[name] = path
    if set(files) != set(expected):
        raise ValueError("ureq vendored file inventory changed")
    return files


def file_hashes(files):
    """Hash only paths returned by checked_files after the full inventory passes."""
    return {name: digest(files[name].read_bytes()) for name in sorted(files)}


def apply_patch(directory, patch, reverse=False):
    # Only a hash-pinned, reviewed patch reaches git. All writes are confined to
    # the temporary reconstruction, never the working tree or index.
    command = ["git", "apply"] + (["--reverse"] if reverse else [])
    subprocess.run(command + [str(patch)], cwd=directory, check=True,
                   capture_output=True, text=True)


def check_sources(root):
    require_real_directory(root / "vendor")
    vendor = root / "vendor/ureq"
    files = checked_files(vendor, RETAINED_FILES | RECORD_FILES)
    record = json.loads(files["UPSTREAM.json"].read_text())
    if (record["package"], record["version"], record["archive_sha256"], record["upstream_git"]) != (
        "ureq", "2.12.1", ARCHIVE_SHA256, UPSTREAM_GIT
    ) or set(record["patched_files"]) != SOURCE_FILES:
        raise ValueError("unexpected ureq source/patch identity")
    if manifest_digest(record["upstream_files"]) != UPSTREAM_FILES_SHA256 \
            or set(record["omitted_files"]) != OMITTED_FILES:
        raise ValueError("ureq published file manifest changed")
    if manifest_digest(record["patched_files"]) != PATCHED_FILES_SHA256 \
            or digest((vendor / PATCH_FILE).read_bytes()) != PATCH_SHA256 \
            or digest((vendor / "response-header.patch").read_bytes()) != AI3_PATCH_SHA256 \
            or manifest_digest(record["historical_ai3_patched_files"]) != AI3_FILES_SHA256:
        raise ValueError("ureq reviewed patch record changed")
    upstream = {name: sha for name, sha in record["upstream_files"].items()
                if name not in OMITTED_FILES}
    expected = upstream | record["patched_files"]
    if set(expected) != RETAINED_FILES:
        raise ValueError("ureq vendored file inventory changed")
    actual = file_hashes({name: files[name] for name in expected})
    for name, sha in expected.items():
        if actual[name] != sha:
            raise ValueError(f"ureq source hash mismatch: {name}")
    with tempfile.TemporaryDirectory(prefix="ureq-provenance-") as temporary:
        clean = Path(temporary) / "clean"
        for name in expected:
            target = clean / name
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(vendor / name, target)
        apply_patch(clean, (vendor / PATCH_FILE).resolve(), reverse=True)
        if file_hashes(checked_files(clean, upstream)) != upstream:
            raise ValueError("ureq cumulative patch does not reconstruct upstream")
        historical = Path(temporary) / "ai3"
        shutil.copytree(clean, historical)
        apply_patch(historical, (vendor / "response-header.patch").resolve())
        ai3 = upstream | record["historical_ai3_patched_files"]
        if file_hashes(checked_files(historical, ai3)) != ai3:
            raise ValueError("ureq historical AI3 patch reconstruction failed")
        apply_patch(clean, (vendor / PATCH_FILE).resolve())
        if file_hashes(checked_files(clean, expected)) != expected:
            raise ValueError("ureq cumulative patch replay differs from selected source")
    return record


def check_archive(archive, record):
    """Optional independent proof against the published crate; no network I/O."""
    if digest(archive.read_bytes()) != ARCHIVE_SHA256:
        raise ValueError("ureq archive identity mismatch")
    files = {}
    with tarfile.open(archive) as package:
        for member in package.getmembers():
            if member.isdir():
                continue
            prefix = "ureq-2.12.1/"
            if not member.isfile() or not member.name.startswith(prefix):
                raise ValueError("unexpected ureq archive member")
            name = member.name[len(prefix):]
            if name in files or ".." in Path(name).parts or Path(name).is_absolute():
                raise ValueError("invalid ureq archive inventory")
            files[name] = digest(package.extractfile(member).read())
    if files != record["upstream_files"]:
        raise ValueError("ureq archive differs from published file manifest")


def check_resolution(root):
    manifest = tomllib.loads((root / "Cargo.toml").read_text())
    dependencies = manifest["workspace"]["dependencies"]
    if dependencies["ureq"]["version"] != "=2.12.1" \
            or manifest["patch"]["crates-io"]["ureq"] != {"path": "vendor/ureq"}:
        raise ValueError("ureq must use the exact local 2.12.1 patch")
    if dependencies["flate2"]["version"] != "=1.1.2" \
            or tomllib.loads((root / "crates/net/Cargo.toml").read_text())["dependencies"]["flate2"] != {"workspace": True}:
        raise ValueError("net must own the pinned shared flate2 decoder")
    lock = tomllib.loads((root / "Cargo.lock").read_text())["package"]
    selected = [p for p in lock if p["name"] == "ureq"]
    if len(selected) != 1 or selected[0]["version"] != "2.12.1" \
            or "source" in selected[0] or "checksum" in selected[0]:
        raise ValueError("locked ureq bypasses the local patch")
    result = subprocess.run(
        # No platform filter: inspect all target edges, workspace features and
        # development dependencies, not just the host/default build lane.
        ["cargo", "metadata", "--all-features", "--locked", "--offline", "--format-version", "1"],
        cwd=root, check=True, capture_output=True, text=True,
    )
    metadata = json.loads(result.stdout)
    packages = metadata["packages"]
    ureq = [p for p in packages if p["name"] == "ureq"]
    if len(ureq) != 1 or ureq[0]["version"] != "2.12.1" \
            or ureq[0]["source"] is not None \
            or Path(ureq[0]["manifest_path"]).resolve() != (root / "vendor/ureq/Cargo.toml").resolve():
        raise ValueError("resolved ureq does not match the reviewed local patch")
    nodes = {node["id"]: node for node in metadata["resolve"]["nodes"]}
    if set(nodes[ureq[0]["id"]]["features"]) != {"tls", "native-certs", "json", "charset"}:
        raise ValueError("unapproved ureq features: automatic decoding must stay disabled")
    rustls = [p for p in packages if p["name"] == "rustls"]
    registry = "registry+https://github.com/rust-lang/crates.io-index"
    if len(rustls) != 1 or rustls[0]["version"] != "0.23.32" \
            or rustls[0]["source"] != registry \
            or set(nodes[rustls[0]["id"]]["features"]) != {"std", "ring", "tls12"}:
        raise ValueError("unapproved rustls selection, provider or logging features")
    flate2 = [p for p in packages if p["name"] == "flate2"]
    net = next(p for p in packages if p["name"] == "net")
    if len(flate2) != 1 or flate2[0]["version"] != "1.1.2" \
            or flate2[0]["source"] != registry \
            or not any(dep["pkg"] == flate2[0]["id"] and dep["name"] == "flate2"
                       and any(kind["kind"] is None for kind in dep["dep_kinds"])
                       for dep in nodes[net["id"]]["deps"]):
        raise ValueError("resolved net decoder does not match flate2 1.1.2")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--archive", type=Path, help="also verify a local published ureq crate archive")
    arguments = parser.parse_args()
    try:
        repository = Path(__file__).resolve().parents[2]
        provenance = check_sources(repository)
        if arguments.archive:
            check_archive(arguments.archive, provenance)
        check_resolution(repository)
        print("ureq 2.12.1: ten-file cumulative patch, upstream reconstruction, locked selection and features verified")
    except (OSError, KeyError, ValueError, StopIteration, tarfile.TarError, subprocess.CalledProcessError) as error:
        sys.exit(str(error))
