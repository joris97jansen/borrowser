#!/usr/bin/env python3
"""Offline history and source identity for Borrowser's two maintained subtrees.

A matching reviewed policy is a qualification input, never owner authorization.
The command verifies a committed candidate; the source functions also support
explicitly labelled working-tree qualification before the final commit.
"""

import sys

if __name__ == "__main__" and (not sys.flags.isolated or not sys.dont_write_bytecode):
    sys.exit("Run this qualification entry point with python3 -I -B (isolated imports, no cache writes).")

import hashlib
import importlib.machinery
import json
import os
from pathlib import Path, PurePosixPath
import stat
import subprocess

ROOT = Path(__file__).resolve().parents[2]
POLICY_PATH = "tools/ci/dependency-subtrees.json"
POLICY_SHA256 = "e9057883fbd9510f2c8547197fa32b404f481941243c6c8059ed136428703b61"
SERVO_BASE = "00a6ce58d02f4e0d43c5ca0702c0bedb8b1ebf3a"
URL_SOURCE = "92f7fb395f9ffb2f706e7c664283781fd3b294a2"
UREQ_RELEASE = "ba4555d54fd72e16fc737f7e8fcc812efb131260"
UREQ_MAINT_RELEASE = "3b0b313db1ab4a08898a904dc7fb04bf4c0b99ae"
ORIGINALS = {
    "rust-url": (URL_SOURCE, "5b1003bbe544b4ee9c25299fbe785289c0831ee7",
                 "https://github.com/servo/rust-url.git", "main"),
    "ureq": (UREQ_RELEASE, "dae21bb12bee845505f16bcdfd34b96033ffef69",
             "https://github.com/algesten/ureq.git", "2.12.x-msrv1.71"),
}
LICENSE_LINKS = {f"{crate}/LICENSE-{license}": f"../LICENSE-{license}"
                 for crate in ("data-url", "idna", "percent_encoding", "url")
                 for license in ("APACHE", "MIT")}
COMMITTED_INPUTS = (
    POLICY_PATH, "tools/ci/check_dependency_subtrees.py",
    "tools/ci/check_url_dependencies.py", "tools/ci/check_ureq_patch.py",
    "Cargo.toml", "Cargo.lock", "fuzz/Cargo.toml", "fuzz/Cargo.lock",
    "Makefile", ".github/workflows/ci.yml", ".github/workflows/rust-url-sync.yml",
    "tools/ci/check_rust_url_ureq_sync.py",
)
# Unlike the fixed entry points above, admit every committed test/helper/fixture
# under this directory, including future additions, and reject uncaptured files.
COMMITTED_TESTS = "tools/ci/tests"
PYTHON_TOOLING = "tools/ci"
# Interpreter-specific ABI names plus portable Unix/macOS and Windows module
# endings: a candidate must remain safe when checked out on either platform.
PYTHON_EXTENSION_SUFFIXES = tuple(sorted({suffix.casefold() for suffix in
                                         importlib.machinery.EXTENSION_SUFFIXES} | {".so", ".pyd"}))


def digest(data):
    return hashlib.sha256(data).hexdigest()


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError("duplicate qualification key")
        result[key] = value
    return result


def real_directory(path):
    if not stat.S_ISDIR(path.lstat().st_mode):
        raise ValueError(f"expected real directory, no symlink: {path}")


def real_file(root, relative):
    path = root
    for part in Path(relative).parts[:-1]:
        path /= part
        real_directory(path)
    path = root / relative
    if not stat.S_ISREG(path.lstat().st_mode):
        raise ValueError(f"expected regular file, no symlink: {path}")
    return path


def python_cache_path(relative):
    path = PurePosixPath(relative)
    return any(part.casefold() == "__pycache__" for part in path.parts) \
        or path.suffix.casefold() in {".pyc", ".pyo"}


def python_extension_path(relative):
    return PurePosixPath(relative).name.casefold().endswith(PYTHON_EXTENSION_SUFFIXES)


def check_python_import_artifacts(root):
    """Reject bytecode/native import artifacts without loading or deleting them."""
    def reject_cache(relative):
        raise ValueError(f"Python bytecode cache prohibited: {relative}. "
                         "Move the cache outside tools/ci before qualification; "
                         "nothing was deleted. -B prevents writes, not cache reads.")

    def reject_extension(relative):
        raise ValueError(f"Native Python extension prohibited: {relative}. "
                         "Move it outside tools/ci and remove tracked/staged copies "
                         "from the reviewed candidate before qualification; nothing was deleted.")

    real_directory(root / "tools")
    real_directory(root / PYTHON_TOOLING)
    pending = [root / PYTHON_TOOLING]
    while pending:
        for path in sorted(pending.pop().iterdir()):
            relative = path.relative_to(root).as_posix()
            if python_cache_path(relative):
                reject_cache(relative)
            if python_extension_path(relative):
                reject_extension(relative)
            mode = path.lstat().st_mode
            if stat.S_ISDIR(mode):
                pending.append(path)
            elif not stat.S_ISREG(mode):
                raise ValueError(f"Nonregular Python tooling entry prohibited: {relative}")
            elif path.suffix.casefold() == ".py" and not relative.startswith(COMMITTED_TESTS + "/") \
                    and relative not in COMMITTED_INPUTS:
                raise ValueError(f"Unqualified Python helper input: {relative}; "
                                 "declare it in COMMITTED_INPUTS before qualification")
    # Include tracked/staged import artifacts even if physically removed.
    committed = git(root, "ls-tree", "-rz", "--name-only", "HEAD", "--", PYTHON_TOOLING).stdout
    indexed = git(root, "ls-files", "-z", "--", PYTHON_TOOLING).stdout
    for row in filter(None, (committed + indexed).split(b"\0")):
        if python_cache_path(row.decode()):
            reject_cache(row.decode())
        if python_extension_path(row.decode()):
            reject_extension(row.decode())
    # A new Python module could also shadow an imported helper or stdlib module.
    untracked = git(root, "ls-files", "--others", "-z", "--", PYTHON_TOOLING).stdout
    for row in filter(None, untracked.split(b"\0")):
        if PurePosixPath(row.decode()).suffix.casefold() == ".py":
            raise ValueError(f"untracked qualification test/helper input: {row.decode()}")


def load_qualification(root):
    data = real_file(root, POLICY_PATH).read_bytes()
    if digest(data) != POLICY_SHA256:
        raise ValueError("qualification record differs from reviewed policy")
    policy = json.loads(data, object_pairs_hook=unique_object)
    if policy["schema"] != 1 or set(policy["dependencies"]) != set(ORIGINALS):
        raise ValueError("unsupported subtree qualification schema")
    if set(policy["history"]) != {f"C{i}" for i in range(1, 6)}:
        raise ValueError("incomplete structural history record")
    for name, original in ORIGINALS.items():
        dep = policy["dependencies"][name]
        if (dep["initial_source"], dep["initial_tree"], dep["repository"], dep["branch"]) != original:
            raise ValueError(f"{name}: original source identity changed")
        if dep["prefix"] != "vendor/" + name or not dep["files"]:
            raise ValueError(f"{name}: invalid prefix or empty source inventory")
        for path, entry in dep["files"].items():
            if str(PurePosixPath(path)) != path or PurePosixPath(path).is_absolute() or ".." in PurePosixPath(path).parts:
                raise ValueError("invalid qualified relative path")
            if entry["mode"] not in {"100644", "100755", "120000"}:
                raise ValueError("unsupported qualified mode")
            if entry["mode"] == "120000" and (
                    name != "rust-url" or LICENSE_LINKS.get(path) != entry.get("target")):
                raise ValueError("unapproved source symlink")
    return policy


def git(root, *args, allowed=(0,)):
    command = ["git", "--no-optional-locks", "-c", "core.hooksPath=/dev/null", "-C", str(root), *args]
    result = subprocess.run(command, capture_output=True, timeout=30)
    if result.returncode not in allowed:
        raise ValueError(f"git {args[0]} failed ({result.returncode}): "
                         + result.stderr.decode(errors="replace")[:1000])
    return result


def text(root, *args):
    return git(root, *args).stdout.decode().strip()


def ancestor(root, left, right):
    return git(root, "merge-base", "--is-ancestor", left, right,
               allowed=(0, 1)).returncode == 0


def git_entries(root, revision, prefix):
    """Read Git modes and blob bytes independently of the checkout/index."""
    tree = f"{revision}:{prefix}" if prefix else revision
    rows = git(root, "ls-tree", "-rz", tree).stdout.split(b"\0")
    entries = []
    for row in filter(None, rows):
        meta, path = row.split(b"\t", 1)
        mode, kind, oid = meta.split()
        if kind != b"blob":
            raise ValueError("subtree contains a non-blob entry")
        entries.append((path.decode(), mode.decode(), oid))
    blobs = b""
    # One batch avoids a process per source file. OIDs, never external paths,
    # form the input protocol; blob lengths delimit arbitrary source bytes.
    if entries:
        result = subprocess.run(["git", "-C", str(root), "cat-file", "--batch"],
                                input=b"".join(oid + b"\n" for _, _, oid in entries),
                                capture_output=True, check=True, timeout=30)
        blobs = result.stdout
    position, result = 0, {}
    for path, mode, oid in entries:
        end = blobs.index(b"\n", position)
        actual, kind, size = blobs[position:end].split()
        if actual != oid or kind != b"blob":
            raise ValueError("invalid Git blob response")
        start = end + 1
        data = blobs[start:start + int(size)]
        position = start + int(size) + 1
        entry = {"mode": mode, "sha256": digest(data)}
        if mode == "120000":
            entry["target"] = data.decode()
        result[path] = entry
    return result


def verify_source(root, name, policy=None):
    """Admit the complete physical inventory before reading source contents."""
    policy = load_qualification(root) if policy is None else policy
    dep = policy["dependencies"][name]
    real_directory(root / "vendor")
    prefix = root / dep["prefix"]
    real_directory(prefix)
    expected = dep["files"]
    directories = {str(parent) for p in expected for parent in PurePosixPath(p).parents
                   if str(parent) != "."}
    admitted = {}
    pending = [prefix]
    while pending:
        directory = pending.pop()
        for path in directory.iterdir():
            relative = path.relative_to(prefix).as_posix()
            mode = path.lstat().st_mode
            if stat.S_ISDIR(mode):
                if relative not in directories:
                    raise ValueError(f"{name}: unexpected source directory")
                pending.append(path)
                continue
            if relative not in expected:
                raise ValueError(f"{name}: unexpected source file")
            if relative in admitted:
                raise ValueError("duplicate source inventory entry")
            entry = expected[relative]
            if entry["mode"] == "120000":
                if not stat.S_ISLNK(mode) or LICENSE_LINKS.get(relative) != entry.get("target"):
                    raise ValueError("expected original license symlink")
                if os.readlink(path) != entry["target"]:
                    raise ValueError("license symlink target changed")
                target = prefix / entry["target"].removeprefix("../")
                if not stat.S_ISREG(target.lstat().st_mode) or path.resolve() != target.resolve():
                    raise ValueError("unsafe license symlink target")
            elif not stat.S_ISREG(mode) or bool(mode & 0o111) != (entry["mode"] == "100755"):
                raise ValueError(f"{name}: source file mode/type changed (symlink or nonregular entry)")
            admitted[relative] = path
    if set(admitted) != set(expected):
        raise ValueError(f"{name}: source inventory changed")
    for relative, path in admitted.items():
        data = os.readlink(path).encode() if path.is_symlink() else path.read_bytes()
        if digest(data) != expected[relative]["sha256"]:
            raise ValueError(f"{name}: source hash mismatch: {relative}")
    return dep


def check_history(root, policy):
    if text(root, "rev-parse", "--is-shallow-repository") != "false":
        raise ValueError("complete Git history required; shallow checkout rejected")
    history = policy["history"]
    c1, c2, c3, c4, c5 = (history[f"C{i}"] for i in range(1, 6))
    pairs = [(c1, [policy["baseline"], URL_SOURCE]), (c2, [c1]),
             (c3, [c2, UREQ_RELEASE]), (c4, [c3, UREQ_MAINT_RELEASE]), (c5, [c4])]
    for commit, parents in pairs:
        if text(root, "show", "-s", "--format=%P", commit).split() != parents:
            raise ValueError("structural commit parent identity changed")
        if not ancestor(root, commit, "HEAD"):
            raise ValueError("structural subtree history is not reachable from HEAD")
    for name, commit in (("rust-url", c1), ("ureq", c3)):
        dep = policy["dependencies"][name]
        if text(root, "rev-parse", dep["initial_source"] + "^{tree}") != dep["initial_tree"]:
            raise ValueError("original source tree identity changed")
        if text(root, "rev-parse", f"{commit}:{dep['prefix']}") != dep["initial_tree"]:
            raise ValueError("pristine import differs from original source tree")
        # The exact prefix tree already proves all imported blobs and modes.
        # Also require the complete first-parent diff to be those additions;
        # an otherwise pristine import must not smuggle in host changes.
        expected = {(b"A", f"{dep['prefix']}/{path}".encode())
                    for path in git_entries(root, dep["initial_source"], "")}
        changes = git(root, "diff", "--no-renames", "--name-status", "-z",
                      commit + "^1", commit).stdout.split(b"\0")[:-1]
        if set(zip(changes[::2], changes[1::2])) != expected:
            raise ValueError(f"{name}: import scope differs from complete upstream inventory")
    if not ancestor(root, SERVO_BASE, URL_SOURCE) or len(
            text(root, "rev-list", f"{SERVO_BASE}..{URL_SOURCE}").splitlines()) != 11:
        raise ValueError("approved eleven-commit Servo contribution history changed")
    if text(root, "rev-parse", UREQ_RELEASE + "^{tree}") != text(root, "rev-parse", UREQ_MAINT_RELEASE + "^{tree}"):
        raise ValueError("ureq release trees differ")
    if text(root, "rev-parse", c3 + "^{tree}") != text(root, "rev-parse", c4 + "^{tree}"):
        raise ValueError("ureq maintenance bridge is not content-neutral")
    original = git_entries(root, policy["baseline"], "vendor/ureq")
    upstream = git_entries(root, c3, "vendor/ureq")
    restored = git_entries(root, c5, "vendor/ureq")
    if len(original) != 54 or len(restored) != 65 or restored != upstream | original:
        raise ValueError("ureq restoration does not preserve original and additional upstream files")
    removed = text(root, "diff", "--name-status", c1, c2).splitlines()
    if len(removed) != 54 or any(not line.startswith("D\tvendor/ureq/") for line in removed):
        raise ValueError("ureq prefix removal includes unexpected changes")
    if any(not p.startswith("vendor/ureq/") for p in text(root, "diff", "--name-only", c4, c5).splitlines()):
        raise ValueError("ureq restoration changed unrelated paths")
    for name, dep in policy["dependencies"].items():
        revision, merge = dep["upstream_revision"], dep["upstream_merge"]
        if not ancestor(root, revision, "HEAD"):
            raise ValueError(f"{name}: qualified upstream revision is unreachable")
        baseline = SERVO_BASE if name == "rust-url" else UREQ_MAINT_RELEASE
        if not ancestor(root, baseline, revision):
            raise ValueError(f"{name}: qualified upstream revision is outside accepted official lineage")
        if merge is not None:
            parents = text(root, "show", "-s", "--format=%P", merge).split()
            if len(parents) != 2 or parents[1] != revision or not ancestor(root, merge, "HEAD"):
                raise ValueError(f"{name}: upstream integration relationship changed")
        elif name != "rust-url" or revision != SERVO_BASE:
            raise ValueError("missing upstream integration commit")


def check_committed_tests(root):
    """Bind test source to HEAD without attesting unrelated local files."""
    real_directory(root / COMMITTED_TESTS)
    if git(root, "diff", "--cached", "--quiet", "HEAD", "--", COMMITTED_TESTS,
           allowed=(0, 1)).returncode:
        raise ValueError("staged qualification tests differ from committed candidate")
    # Deliberately omit --exclude-standard: ignored source is still source.
    untracked = git(root, "ls-files", "--others", "-z", "--", COMMITTED_TESTS).stdout
    for row in filter(None, untracked.split(b"\0")):
        relative = row.decode()
        raise ValueError(f"untracked qualification test input: {relative}")
    for relative, entry in git_entries(root, "HEAD", COMMITTED_TESTS).items():
        path = real_file(root, f"{COMMITTED_TESTS}/{relative}")
        mode = "100755" if path.stat().st_mode & 0o111 else "100644"
        if mode != entry["mode"] or digest(path.read_bytes()) != entry["sha256"]:
            raise ValueError(f"uncommitted qualification test: {relative}")


def check_committed_input(root, relative):
    actual = real_file(root, relative).read_bytes()
    if git(root, "show", f"HEAD:{relative}").stdout != actual:
        raise ValueError(f"uncommitted qualification input: {relative}")
    if git(root, "diff", "--cached", "--quiet", "HEAD", "--", relative, allowed=(0, 1)).returncode:
        raise ValueError(f"staged qualification input: {relative}")


def check_python_inputs(root):
    check_python_import_artifacts(root)
    for relative in COMMITTED_INPUTS:
        if relative.endswith(".py"):
            check_committed_input(root, relative)
    check_committed_tests(root)


def check_repo(root):
    check_python_inputs(root)
    policy = load_qualification(root)
    check_history(root, policy)
    for name, dep in policy["dependencies"].items():
        if git_entries(root, "HEAD", dep["prefix"]) != dep["files"]:
            raise ValueError(f"{name}: committed source differs from qualification")
        staged = git(root, "ls-files", "--stage", "-z", "--", dep["prefix"]).stdout
        if any(row.split(b"\t", 1)[0].split()[2] != b"0" for row in staged.split(b"\0") if row):
            raise ValueError("unresolved index entries")
        if git(root, "diff", "--cached", "--quiet", "HEAD", "--", dep["prefix"], allowed=(0, 1)).returncode:
            raise ValueError("staged vendor changes differ from committed candidate")
        verify_source(root, name, policy)
    for relative in COMMITTED_INPUTS:
        if not relative.endswith(".py"):
            check_committed_input(root, relative)
    return policy


if __name__ == "__main__":
    try:
        if sys.argv[1:] == ["--python-preflight"]:
            check_python_inputs(ROOT)
            print("Python qualification inputs: committed source, no bytecode caches or native extensions")
        elif sys.argv[1:] in ([], ["--tests"]):
            check_repo(ROOT)
            print("rust-url and ureq: committed history, source, tests, index and physical checkout verified",
                  flush=True)
            if sys.argv[1:]:
                # Only now may discovery import repository-local test/helpers.
                import unittest
                sys.path.insert(0, str(ROOT / PYTHON_TOOLING))
                suite = unittest.defaultTestLoader.discover(str(ROOT / COMMITTED_TESTS))
                if not suite.countTestCases():
                    raise ValueError("No qualification tests discovered")
                result = unittest.TextTestRunner(verbosity=2).run(suite)
                check_repo(ROOT)
                sys.exit(not result.wasSuccessful())
        else:
            sys.exit("Usage: python3 -I -B tools/ci/check_dependency_subtrees.py [--python-preflight|--tests]")
    except (ValueError, OSError, KeyError, TypeError, subprocess.SubprocessError) as error:
        sys.exit(str(error))
