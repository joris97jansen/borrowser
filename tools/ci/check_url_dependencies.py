#!/usr/bin/env python3
"""Verify the exact approved rust-url snapshot and both offline Cargo graphs."""

import argparse
import hashlib
import json
import os
import shutil
import stat
import tempfile
from pathlib import Path
import subprocess
import sys
import tomllib

# Reviewed CI policy, independent of the manifests/locks being checked.
# Qualification graph lock SHA-256:
# a770f264be9656bcf64db971b60c2a09ee0a542600ead12b66f52f878db154d6
# AH8 source qualification (2026-11-04), completed before advancing this policy:
# https://github.com/joris97jansen/rust-url/actions/runs/41777938203
# CI control 823ba428f3a9883e8873e91d6cf46b7ac822c0a7 is not the parser source.
# Update only after dependency requalification; never substitute its CI-control SHA.
APPROVED_REVISION = "92f7fb395f9ffb2f706e7c664283781fd3b294a2"
APPROVED_REPOSITORY = "https://github.com/joris97jansen/rust-url.git"
APPROVED_SOURCE = f"git+{APPROVED_REPOSITORY}?rev={APPROVED_REVISION}#{APPROVED_REVISION}"
REGISTRY_SOURCE = "registry+https://github.com/rust-lang/crates.io-index"
# Cargo 1.92 RegistrySource::short_name for the official sparse and Git indexes.
# Cache layout changes require explicit qualification, not a permissive fallback.
REGISTRY_CACHE_NAMES = {"index.crates.io-1949cf8c6b5b557f", "github.com-25cdd57fae9f0462"}
FORK_VERSIONS = {
    "url": "2.5.8",
    "idna": "1.1.0",
    "form_urlencoded": "1.2.2",
    "percent-encoding": "2.3.2",
}
REGISTRY_PACKAGES = {
    'displaydoc': ('0.2.5', '97369cbbc041bc366949bc74d34658d6cda5621039731c6310521892a3a20ae0'),
    'icu_collections': ('2.0.0', '200072f5d0e3614556f94a9930d5dc3e0662a652823904c3a75dc3b0af7fee47'),
    'icu_locale_core': ('2.0.0', '0cde2700ccaed3872079a65fb1a78f6c0a36c91570f28755dda67bc8f7d9f00a'),
    'icu_normalizer': ('2.0.0', '436880e8e18df4d7bbc06d58432329d6458cc84531f7ac5f024e93deadb37979'),
    'icu_normalizer_data': ('2.0.0', '00210d6893afc98edb752b664b8890f0ef174c8adbb8d0be9710fa66fbbf72d3'),
    'icu_properties': ('2.0.1', '016c619c1eeb94efb86809b015c58f479963de65bdb6253345c1a1276f22e32b'),
    'icu_properties_data': ('2.0.1', '298459143998310acd25ffe6810ed544932242d3f07083eee1084d83a71bd632'),
    'icu_provider': ('2.0.0', '03c80da27b5f4187909049ee2d72f276f0d9f99a42c306bd0131ecfe04d8e5af'),
    'idna_adapter': ('1.2.1', '3acae9609540aa318d1bc588455225fb2085b9ed0c4f6bd0d9d5bcd86f1a0344'),
    'litemap': ('0.8.0', '241eaef5fd12c88705a01fc1066c48c4b36e0dd4377dcdc7ec3942cea7a69956'),
    'potential_utf': ('0.1.3', '84df19adbe5b5a0782edcab45899906947ab039ccf4573713735ee7de1e6b08a'),
    'proc-macro2': ('1.0.101', '89ae43fd86e4158d6db51ad8e2b80f313af9cc74f5c0e03ccb87de09998732de'),
    'quote': ('1.0.40', '1885c039570dc00dcb4ff087a89e185fd56bae234ddc7f056a945bf36467248d'),
    'serde': ('1.0.225', 'fd6c24dee235d0da097043389623fb913daddf92c76e9f5a1db88607a0bcbd1d'),
    'serde_core': ('1.0.225', '659356f9a0cb1e529b24c01e43ad2bdf520ec4ceaf83047b83ddcc2251f96383'),
    'serde_derive': ('1.0.225', '0ea936adf78b1f766949a4977b91d2f5595825bd6ec079aa9543ad2685fc4516'),
    'smallvec': ('1.15.1', '67b1b7a3b5fe4f1376887184045fcf45c69e92af734b7aaddc05fb777b6fbd03'),
    'stable_deref_trait': ('1.2.0', 'a8f112729512f8e442d81f95a8a7ddf2b7c6b8a1a6f509a95864142b30cab2d3'),
    'syn': ('2.0.106', 'ede7c438028d4436d71104916910f5bb611972c5cfd7f89b8300a8186e6fada6'),
    'synstructure': ('0.13.2', '728a70f3dbaf5bab7f0c4b1ac8d7ae5ea60a4b5549c8a5914361c99147a709d2'),
    'tinystr': ('0.8.1', '5d4f6d1145dcb577acf783d4e601bc1d76a13337bb54e6233add580b07344c8b'),
    'unicode-ident': ('1.0.19', 'f63a545481291138910575129486daeaf8ac54aee4387fe7906919f7830c7d9d'),
    'utf8_iter': ('1.0.4', 'b6c140620e7ffbb22c2dee59cafe6084a59b5ffc27a8859a5f0d494b5d52b6be'),
    'writeable': ('0.6.1', 'ea2f10b9bb0928dfb1b42b65e1f9e36f7f54dbdf08457afefb38afcdec4fa2bb'),
    'yoke': ('0.8.0', '5f41bb01b8226ef4bfd589436a297c53d118f65921786300e427be8d487695cc'),
    'yoke-derive': ('0.8.0', '38da3c9736e16c5d3c8c597a9aaa5d1fa565d0532ae05e27c24aa62fb32c0ab6'),
    'zerofrom': ('0.1.6', '50cc42e0333e05660c3587f3bf9d0478688e15d870fab3346451ce7f8c9fbea5'),
    'zerofrom-derive': ('0.1.6', 'd71e5d6e06ab090c67b5e44993ec16b72dcbaabc526db883a360057678b48502'),
    'zerotrie': ('0.2.2', '36f0bbd478583f79edad978b407914f61b2972f5af6fa089686016be8f9af595'),
    'zerovec': ('0.11.4', 'e7aa2bd55086f1ab526693ecbe444205da57e25f4489879da80635a46d90e73b'),
    'zerovec-derive': ('0.11.1', '5b96237efa0c878c64bd89c436f661be4e46b2f3eff1ebb976f7ef2321d2f58f'),
}


# Source-specific policy; advancing these anchors requires explicit requalification.
UPSTREAM_REVISION = "00a6ce58d02f4e0d43c5ca0702c0bedb8b1ebf3a"
UPSTREAM_TREE = "e14f83138396ca6f64702f263516ba5c39c9ffa9"
APPROVED_TREE = "5b1003bbe544b4ee9c25299fbe785289c0831ee7"
RECORD_HASHES = {
    "UPSTREAM.json": "ef5186d14d335d9430b9e7881427f0fbd9e4f4a1fe528b129ddd3dcd99ec5583",
    "fork.patch": "4c2d43bd7bcf39438bc975087c20e087341aa470393a1a3147a53fdef1813d47",
    "QUALIFIED-GRAPHS.json": "63320bbbc9a1eaf9e09fd897665176a86b87ca7523715f807df17ad34f8490b9",
    "BORROWSER-PATCH.md": "b25b38033829afc4a83da3e5dfec63a202ea0e66c491e25235e7f93f87bd9e45",
}
LAYOUT_SHA256 = "0b211d376a5925ac9ada0d20de2eeaf6b325db48edff930d386133631047e0bd"
LOCAL_CRATES = {"url": "url", "idna": "idna", "form_urlencoded": "form_urlencoded",
                "percent-encoding": "percent_encoding"}
ROOT = Path(__file__).resolve().parents[2]


def digest(data):
    return hashlib.sha256(data).hexdigest()


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(",", ":")).encode()


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError("duplicate provenance key")
        result[key] = value
    return result


def read_json(data):
    return json.loads(data, object_pairs_hook=unique_object)


def require_directory(path):
    mode = path.lstat().st_mode
    if stat.S_ISLNK(mode) or not stat.S_ISDIR(mode):
        raise ValueError(f"expected real directory, no symlink: {path}")


def require_file(root, relative):
    path = root
    for part in Path(relative).parts[:-1]:
        path = path / part
        require_directory(path)
    path = root / relative
    mode = path.lstat().st_mode
    if stat.S_ISLNK(mode) or not stat.S_ISREG(mode):
        raise ValueError(f"expected regular file, no symlink: {path}")
    return path


def inventory(directory):
    """Admit entry types without following links or opening any file contents."""
    require_directory(directory)
    files, layout = {}, {}
    pending = [directory]
    while pending:
        parent = pending.pop()
        for path in parent.iterdir():
            name = path.relative_to(directory).as_posix()
            if name in layout:
                raise ValueError("duplicate rust-url inventory entry")
            mode = path.lstat().st_mode
            if stat.S_ISLNK(mode):
                raise ValueError("rust-url inventory contains a symlink")
            if stat.S_ISDIR(mode):
                layout[name] = "directory"
                pending.append(path)
            elif stat.S_ISREG(mode):
                # Every approved Git file and materialized license is 100644.
                if mode & 0o111:
                    raise ValueError("rust-url inventory contains an unexpected executable file")
                layout[name] = "file"
                files[name] = path
            else:
                raise ValueError("rust-url inventory contains a nonregular entry")
    return files, layout


def file_hashes(files):
    return {name: digest(path.read_bytes()) for name, path in sorted(files.items())}


def apply_patch(directory, patch, reverse=False):
    # Pinned patch; no index operation; writes only to disposable reconstruction.
    subprocess.run(["git", "-c", "core.autocrlf=false", "apply", "--whitespace=nowarn",
                    *(["--reverse"] if reverse else []), str(patch)],
                   cwd=directory, capture_output=True, text=True, check=True)


def verify_contributions(record):
    known = {UPSTREAM_REVISION}
    parents = {}
    for commit in record["contributions"]:
        raw = commit["object"].encode()
        identity = hashlib.sha1(b"commit " + str(len(raw)).encode() + b"\0" + raw).hexdigest()
        if identity != commit["revision"] or identity in known:
            raise ValueError("invalid rust-url contribution object")
        head = commit["object"].split("\n\n", 1)[0].splitlines()
        links = [line.removeprefix("parent ") for line in head if line.startswith("parent ")]
        if not links or any(parent not in known for parent in links):
            raise ValueError("incomplete rust-url contribution topology")
        parents[identity] = links
        known.add(identity)
    pending, ancestors = [APPROVED_REVISION], set()
    while pending:
        identity = pending.pop()
        if identity == UPSTREAM_REVISION or identity in ancestors:
            continue
        ancestors.add(identity)
        pending.extend(parents[identity])
    if ancestors != set(parents) or len(parents) != 11:
        raise ValueError("unexpected rust-url contribution inventory")


def check_sources(root):
    require_directory(root / "vendor")
    vendor = root / "vendor/rust-url"
    files, layout = inventory(vendor)
    if digest(canonical(layout)) != LAYOUT_SHA256:
        raise ValueError("rust-url exact file/directory inventory changed")
    for name, expected in RECORD_HASHES.items():
        if digest(files[name].read_bytes()) != expected:
            raise ValueError(f"rust-url frozen record mismatch: {name}")
    record = read_json(files["UPSTREAM.json"].read_bytes())
    if record["schema"] != 1 or (record["upstream"]["revision"], record["upstream"]["tree"],
                               record["fork"]["revision"], record["fork"]["tree"]) != (
            UPSTREAM_REVISION, UPSTREAM_TREE, APPROVED_REVISION, APPROVED_TREE):
        raise ValueError("unapproved rust-url source identity")
    verify_contributions(record)
    for section in ("upstream", "fork"):
        snapshot = record[section]
        for name, entry in snapshot["git_files"].items():
            if entry["mode"] == "120000":
                target = snapshot["license_symlinks"][name]
                if target not in ("../LICENSE-MIT", "../LICENSE-APACHE"):
                    raise ValueError("unsupported license transformation")
                content = target.encode()
                expected = snapshot["materialized_files"][target.removeprefix("../")]
            elif entry["mode"] == "100644":
                content = None
                expected = entry["sha256"]
            else:
                raise ValueError("unsupported original Git entry")
            if snapshot["materialized_files"][name] != expected:
                raise ValueError("inconsistent materialized source record")
            if content is not None and (digest(content) != entry["sha256"] or
                    hashlib.sha1(b"blob " + str(len(content)).encode() + b"\0" + content).hexdigest() != entry["blob"]):
                raise ValueError("invalid original license link identity")
    expected = record["fork"]["materialized_files"]
    if set(files) != set(expected) | set(RECORD_HASHES):
        raise ValueError("inconsistent rust-url source/record inventory")
    if file_hashes({name: files[name] for name in expected}) != expected:
        raise ValueError("rust-url source hash mismatch")
    with tempfile.TemporaryDirectory(prefix="rust-url-provenance-") as temporary:
        clean = Path(temporary)
        for name in expected:
            target = clean / name
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(files[name], target)
        patch = files["fork.patch"].resolve()
        apply_patch(clean, patch, reverse=True)
        if file_hashes(inventory(clean)[0]) != record["upstream"]["materialized_files"]:
            raise ValueError("rust-url patch does not reconstruct upstream")
        apply_patch(clean, patch)
        if file_hashes(inventory(clean)[0]) != expected:
            raise ValueError("rust-url patch replay differs from approved fork")
    return record


def load_resolution(manifest, arguments=()):
    """Inspect actual locked selection offline; never repair a manifest or lock."""
    lock_path = manifest.with_name("Cargo.lock")
    before = lock_path.read_bytes()
    try:
        result = subprocess.run(
            ["cargo", "metadata", "--offline", "--locked", "--format-version", "1",
             "--manifest-path", str(manifest), *arguments],
            check=True, capture_output=True, text=True,
        )
    finally:
        if lock_path.read_bytes() != before:
            raise ValueError(f"{manifest}: locked metadata changed Cargo.lock")
    if result.stderr:
        print(result.stderr, end="", file=sys.stderr)
    return read_json(result.stdout), tomllib.loads(before.decode())


def check_workspace_records(root, label):
    """Static configuration shared with the observational monitor, without Cargo."""
    if label not in ("main", "fuzz"):
        raise ValueError("unsupported rust-url consumer workspace")
    directory = root if label == "main" else root / "fuzz"
    relative = "" if label == "main" else "fuzz/"
    manifest = tomllib.loads(require_file(root, relative + "Cargo.toml").read_text())
    lock = tomllib.loads(require_file(root, relative + "Cargo.lock").read_text())
    expected = {"url": {"path": "vendor/rust-url/url"},
                "percent-encoding": {"path": "vendor/rust-url/percent_encoding"}} if label == "main" else {
                    "url": {"path": "../vendor/rust-url/url"}}
    patches = manifest.get("patch", {}).get("crates-io", {})
    if {name: declaration for name, declaration in patches.items() if name in FORK_VERSIONS} != expected:
        raise ValueError(f"{label}: unsupported rust-url path patch configuration")
    if label == "main" and "vendor/rust-url" not in manifest["workspace"].get("exclude", []):
        raise ValueError("main: nested rust-url workspace must be excluded")
    for dependencies in (manifest.get("dependencies", {}),
                         manifest.get("workspace", {}).get("dependencies", {})):
        for name in FORK_VERSIONS:
            declaration = dependencies.get(name)
            if declaration is not None and not isinstance(declaration, (str, dict)):
                raise ValueError(f"{label}: unsupported {name} declaration")
            if isinstance(declaration, dict) and ({"path", "git", "rev", "branch", "tag", "registry"} & declaration.keys()
                                                  or declaration.get("package", name) != name):
                raise ValueError(f"{label}: source override outside approved patches")
    for name, version in FORK_VERSIONS.items():
        selected = [p for p in lock["package"] if p["name"] == name]
        if len(selected) != 1 or selected[0]["version"] != version or {"source", "checksum"} & selected[0].keys():
            raise ValueError(f"{label}: locked {name} must select the approved local version")
        require_file(root, "vendor/rust-url/" + LOCAL_CRATES[name] + "/Cargo.toml")
    if any("github.com/joris97jansen/rust-url" in p.get("source", "") for p in lock["package"]):
        raise ValueError(f"{label}: unexpected residual Git source")
    return directory / "Cargo.toml", lock


def load_graph_policy(root):
    path = require_file(root, "vendor/rust-url/QUALIFIED-GRAPHS.json")
    data = path.read_bytes()
    if digest(data) != RECORD_HASHES["QUALIFIED-GRAPHS.json"]:
        raise ValueError("rust-url frozen graph record mismatch")
    return read_json(data)


def verify_registry_origin(package, label):
    """Approve the effective location, not just Cargo's replacement-preserved ID.

    Cargo 1.92 keeps registry source IDs for DirectorySource replacements. Its
    editable .cargo-checksum.json is mandatory even inside a registry cache;
    it is not independent authorization. This checks location/entry types,
    not the cryptographic integrity of every extracted registry source file.
    """
    name = package["name"]
    try:
        if package["source"] != REGISTRY_SOURCE:
            raise ValueError("unexpected registry identity")
        # Match home::cargo_home_with_cwd: empty/unset uses HOME/.cargo;
        # relative CARGO_HOME is relative to the Cargo invocation's cwd.
        home = Path(os.path.abspath(os.environ.get("CARGO_HOME") or Path.home() / ".cargo"))
        real_home = home.resolve(strict=True)
        require_directory(real_home)
        manifest = Path(package["manifest_path"])
        # Resolve the explicitly configured home boundary only. Do not resolve
        # away symlinks in the cache or package path before admitting entries.
        boundary = home if manifest.is_relative_to(home) else real_home
        relative = manifest.relative_to(boundary)
        parts = relative.parts
        if (len(parts) != 5 or parts[:2] != ("registry", "src")
                or parts[2] not in REGISTRY_CACHE_NAMES
                or parts[3:] != (f"{name}-{package['version']}", "Cargo.toml")):
            raise ValueError("outside approved crates.io extraction layout")
        require_file(boundary, relative)
        if os.path.lexists(manifest.with_name(".cargo-checksum.json")):
            raise ValueError("directory-source checksum record is not approved")
    except (OSError, ValueError, TypeError, RuntimeError) as error:
        raise ValueError(f"{label}: unapproved registry origin for {name}: {error}") from error


def verify_resolution(metadata, lock, label, root=ROOT, expected_graph=None):
    if metadata.get("version") != 1 or not isinstance(metadata.get("resolve"), dict):
        raise ValueError(f"{label}: unsupported Cargo metadata")
    packages = {p["id"]: p for p in metadata["packages"]}
    nodes = {n["id"]: n for n in metadata["resolve"]["nodes"]}
    if len(packages) != len(metadata["packages"]) or len(nodes) != len(metadata["resolve"]["nodes"]):
        raise ValueError(f"{label}: duplicate Cargo identity")
    if set(nodes) != set(packages):
        raise ValueError(f"{label}: incomplete Cargo resolution")
    if any(Path(packages[member]["manifest_path"]).is_relative_to(root / "vendor/rust-url")
           for member in metadata["workspace_members"]):
        raise ValueError(f"{label}: vendored crate joined consumer workspace")
    for name, version in FORK_VERSIONS.items():
        selected = [p for p in packages.values() if p["name"] == name]
        if len(selected) != 1:
            raise ValueError(f"{label}: expected one {name} instance, found {len(selected)}")
        package = selected[0]
        expected_path = require_file(root, "vendor/rust-url/" + LOCAL_CRATES[name] + "/Cargo.toml")
        if package["source"] is not None or Path(package["manifest_path"]) != expected_path:
            raise ValueError(f"{label}: {name} must resolve from the approved local manifest")
        if package["version"] != version:
            raise ValueError(f"{label}: unexpected {name} version {package['version']}")
    if any("github.com/joris97jansen/rust-url" in (p.get("source") or "") for p in packages.values()):
        raise ValueError(f"{label}: unexpected Git fallback")
    pending = [p["id"] for p in packages.values() if p["name"] == "url"]
    closure = set()
    try:
        while pending:
            package_id = pending.pop()
            if package_id in closure:
                continue
            closure.add(package_id)
            pending.extend(d["pkg"] for d in nodes[package_id]["deps"]
                           if any(kind["kind"] != "dev" for kind in d["dep_kinds"]))
        actual = {(packages[p]["name"], packages[p]["version"]) for p in closure}
    except KeyError as error:
        raise ValueError(f"{label}: incomplete Cargo dependency edge") from error
    expected = set(FORK_VERSIONS.items()) | {(n, v) for n, (v, _) in REGISTRY_PACKAGES.items()}
    if actual != expected or len(closure) != len(expected):
        raise ValueError(f"{label}: qualified URL closure changed")
    locked = {(p["name"], p["version"], p.get("source")): p for p in lock["package"]}
    if len(locked) != len(lock["package"]):
        raise ValueError(f"{label}: duplicate locked package identity")
    projection = []
    for package_id in closure:
        package = packages[package_id]
        name = package["name"]
        recorded = locked.get((name, package["version"], package["source"]))
        if recorded is None:
            raise ValueError(f"{label}: selected package missing from lock: {name}")
        if name in FORK_VERSIONS:
            if "source" in recorded or "checksum" in recorded:
                raise ValueError(f"{label}: nonlocal locked source: {name}")
        else:
            if package["source"] != REGISTRY_SOURCE or recorded.get("checksum") != REGISTRY_PACKAGES[name][1]:
                raise ValueError(f"{label}: qualified checksum mismatch for {name}")
            verify_registry_origin(package, label)
        node = nodes[package_id]
        edges = []
        for dep in node["deps"]:
            kinds = [kind for kind in dep["dep_kinds"] if kind["kind"] != "dev"]
            if kinds:
                edges.append({"name": dep["name"], "package": packages[dep["pkg"]]["name"],
                              "kinds": sorted(kinds, key=canonical)})
        projection.append({"package": name, "features": sorted(node["features"]),
                           "edges": sorted(edges, key=canonical)})
    if expected_graph is None:
        expected_graph = load_graph_policy(root)["graphs"][label]
    if sorted(projection, key=lambda n: n["package"]) != expected_graph:
        raise ValueError(f"{label}: qualified dependency edges or features changed")


def check_workspace(manifest, label, arguments=(), expected_graph=None):
    root = manifest.parent if label == "main" else manifest.parent.parent
    metadata, lock = load_resolution(manifest, arguments)
    verify_resolution(metadata, lock, label, root, expected_graph)
    print(f"{label} {list(arguments)}: exact local source, registry origins, 35-package graph, edges and features verified")


def check_repo(root):
    check_sources(root)
    policy = load_graph_policy(root)
    manifests = {label: check_workspace_records(root, label)[0] for label in ("main", "fuzz")}
    for name, configuration in sorted(policy["configurations"].items(), key=lambda p: (p[1]["workspace"] != "main", p[0])):
        label = configuration["workspace"]
        check_workspace(manifests[label], label, configuration["arguments"], policy["graphs"][label])


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-only", action="store_true")
    arguments = parser.parse_args()
    try:
        if arguments.source_only:
            check_sources(ROOT)
            print("rust-url: approved materialized source and upstream reconstruction verified")
        else:
            check_repo(ROOT)
    except subprocess.CalledProcessError as error:
        print(error.stderr, file=sys.stderr)
        sys.exit(error.returncode)
    except (OSError, ValueError, KeyError, TypeError, AttributeError) as error:
        sys.exit(str(error))
