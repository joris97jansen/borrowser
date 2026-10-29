#!/usr/bin/env python3
"""Enforce the AH3-qualified URL graph in both Cargo workspaces (Python 3.11+)."""

import json
from pathlib import Path
import subprocess
import sys
import tomllib

# Reviewed CI policy, independent of the manifests/locks being checked.
# Qualification graph lock SHA-256:
# a770f264be9656bcf64db971b60c2a09ee0a542600ead12b66f52f878db154d6
# Update only after dependency requalification; never substitute its CI-control SHA.
APPROVED_REVISION = "7dc4ecbf4d29817e723e04ae94b14f1326764f61"
APPROVED_REPOSITORY = "https://github.com/joris97jansen/rust-url.git"
APPROVED_SOURCE = f"git+{APPROVED_REPOSITORY}?rev={APPROVED_REVISION}#{APPROVED_REVISION}"
REGISTRY_SOURCE = "registry+https://github.com/rust-lang/crates.io-index"
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


def load_resolution(manifest):
    """Ask Cargo for the actual selection; never infer Git identity from the lock."""
    lock_path = manifest.with_name("Cargo.lock")
    before = lock_path.read_bytes()
    result = subprocess.run(
        ["cargo", "metadata", "--locked", "--format-version", "1",
         "--manifest-path", str(manifest)],
        check=True, capture_output=True, text=True,
    )
    if result.stderr:
        print(result.stderr, end="", file=sys.stderr)
    after = lock_path.read_bytes()
    if after != before:
        raise ValueError(f"{manifest}: locked metadata changed Cargo.lock")
    return json.loads(result.stdout), tomllib.loads(after.decode())


def verify_resolution(metadata, lock, label):
    packages = {p["id"]: p for p in metadata["packages"]}
    for name, version in FORK_VERSIONS.items():
        selected = [p for p in packages.values() if p["name"] == name]
        if len(selected) != 1:
            raise ValueError(f"{label}: expected one {name} instance, found {len(selected)}")
        package = selected[0]
        if package["source"] != APPROVED_SOURCE:
            raise ValueError(f"{label}: {name} must resolve from the approved fork at {APPROVED_REVISION}")
        if package["version"] != version:
            raise ValueError(f"{label}: unexpected {name} version {package['version']}")

    nodes = {node["id"]: node for node in metadata["resolve"]["nodes"]}
    pending = [p["id"] for p in packages.values() if p["name"] == "url"]
    closure = set()
    while pending:
        package_id = pending.pop()
        if package_id in closure:
            continue
        closure.add(package_id)
        pending.extend(
            dep["pkg"] for dep in nodes[package_id]["deps"]
            if any(kind["kind"] != "dev" for kind in dep["dep_kinds"])
        )

    actual = {(packages[p]["name"], packages[p]["version"]) for p in closure}
    expected = set(FORK_VERSIONS.items()) | {
        (name, version) for name, (version, _) in REGISTRY_PACKAGES.items()
    }
    if actual != expected or len(closure) != len(expected):
        raise ValueError(
            f"{label}: qualified URL closure changed; "
            f"missing={sorted(expected - actual)}, unexpected={sorted(actual - expected)}"
        )

    # Metadata supplies the resolved identity; the lock supplies its registry
    # checksum, which must match the separately frozen qualification policy.
    locked = {(p["name"], p["version"], p.get("source")): p for p in lock["package"]}
    for package_id in sorted(closure):
        package = packages[package_id]
        name = package["name"]
        if name in FORK_VERSIONS:
            continue
        if package["source"] != REGISTRY_SOURCE:
            raise ValueError(f"{label}: unexpected registry source for {name}")
        coordinate = (name, package["version"], package["source"])
        recorded = locked.get(coordinate)
        if recorded is None or recorded.get("checksum") != REGISTRY_PACKAGES[name][1]:
            raise ValueError(f"{label}: qualified checksum mismatch for {name}")


def check_workspace(manifest, label):
    metadata, lock = load_resolution(manifest)
    verify_resolution(metadata, lock, label)
    print(f"{label}: approved URL revision and 35-package dependency closure verified")


def check_repo(root):
    check_workspace(root / "Cargo.toml", "main")
    check_workspace(root / "fuzz/Cargo.toml", "fuzz")


if __name__ == "__main__":
    try:
        check_repo(Path(__file__).resolve().parents[2])
    except subprocess.CalledProcessError as error:
        print(error.stderr, file=sys.stderr)
        sys.exit(error.returncode)
    except (OSError, ValueError) as error:
        sys.exit(str(error))
