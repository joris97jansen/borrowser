#!/usr/bin/env python3
"""Verify Borrowser's exact, locally patched ureq 2.12.1 dependency."""

import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tomllib

ARCHIVE_SHA256 = "02d1a66277ed75f640d608235660df48c8e3c19f3b4edb6a263315626cc3c01d"
UPSTREAM_FILES_SHA256 = "6c820757679ccd8200e73c04286f3d77d8b1e7af3af3c6c98c36a4f8f6932da3"
PATCH_SHA256 = "0c2eb5cda5ecf67a476457de2dffefbdb82c910f7c8f49b1d07ba874dfd9d969"
PATCHED_FILES_SHA256 = "2c75dfc6b1e10a27fa2458db2a80e2e87e032eaaa8d376c0e78322a4f1ab5ef3"
SOURCE_FILES = {"src/header.rs", "src/response.rs"}
RECORD_FILES = {"UPSTREAM.json", "BORROWSER-PATCH.md", "response-header.patch"}
OMITTED_FILES = {".github/dependabot.yml", ".github/workflows/test.yml", ".gitignore", "Cargo.lock"}


def check_sources(root):
    vendor = root / "vendor/ureq"
    record = json.loads((vendor / "UPSTREAM.json").read_text())
    if (record["package"], record["version"], record["archive_sha256"]) != (
        "ureq", "2.12.1", ARCHIVE_SHA256
    ) or set(record["patched_files"]) != SOURCE_FILES:
        raise ValueError("unexpected ureq source/patch identity")
    upstream = json.dumps(record["upstream_files"], sort_keys=True, separators=(",", ":")).encode()
    if hashlib.sha256(upstream).hexdigest() != UPSTREAM_FILES_SHA256 \
            or set(record["omitted_files"]) != OMITTED_FILES:
        raise ValueError("ureq published file manifest changed")
    patched = json.dumps(record["patched_files"], sort_keys=True, separators=(",", ":")).encode()
    if hashlib.sha256(patched).hexdigest() != PATCHED_FILES_SHA256 \
            or hashlib.sha256((vendor / "response-header.patch").read_bytes()).hexdigest() != PATCH_SHA256:
        raise ValueError("ureq reviewed patch record changed")
    expected = dict(record["upstream_files"])
    for name in record["omitted_files"]:
        expected.pop(name)
    expected.update(record["patched_files"])
    actual = {str(p.relative_to(vendor)) for p in vendor.rglob("*") if p.is_file()}
    if actual != set(expected) | RECORD_FILES:
        raise ValueError("ureq vendored file inventory changed")
    for name, digest in expected.items():
        if hashlib.sha256((vendor / name).read_bytes()).hexdigest() != digest:
            raise ValueError(f"ureq source hash mismatch: {name}")
    # Check that the reviewed patch still describes these exact source hunks.
    subprocess.run(
        ["git", "apply", "--reverse", "--check", "--directory=vendor/ureq",
         "vendor/ureq/response-header.patch"], cwd=root, check=True,
        capture_output=True, text=True,
    )


def check_resolution(root):
    manifest = tomllib.loads((root / "Cargo.toml").read_text())
    if manifest["workspace"]["dependencies"]["ureq"]["version"] != "=2.12.1" \
            or manifest["patch"]["crates-io"]["ureq"] != {"path": "vendor/ureq"}:
        raise ValueError("ureq must use the exact local 2.12.1 patch")
    result = subprocess.run(
        ["cargo", "metadata", "--locked", "--offline", "--format-version", "1"],
        cwd=root, check=True, capture_output=True, text=True,
    )
    packages = [p for p in json.loads(result.stdout)["packages"] if p["name"] == "ureq"]
    if len(packages) != 1 or packages[0]["version"] != "2.12.1" \
            or packages[0]["source"] is not None \
            or Path(packages[0]["manifest_path"]).resolve() != (root / "vendor/ureq/Cargo.toml").resolve():
        raise ValueError("resolved ureq does not match the reviewed local patch")


if __name__ == "__main__":
    try:
        repository = Path(__file__).resolve().parents[2]
        check_sources(repository)
        check_resolution(repository)
        print("ureq 2.12.1: pinned source, two-file patch and locked selection verified")
    except (OSError, KeyError, ValueError, subprocess.CalledProcessError) as error:
        sys.exit(str(error))
