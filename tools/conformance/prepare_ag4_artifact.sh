#!/bin/sh
# Standard Git temporary-index snapshot, then two builds from that exact tree.
set -eu
exec python3 - "$0" "$@" <<'PY'
import argparse, hashlib, json, os, pathlib, shutil, ssl, stat, sys, tempfile
sys.dont_write_bytecode = True
sys.path.insert(0, str(pathlib.Path(sys.argv.pop(1)).resolve().parent))
from prepare_ag4_process import Preparation

p = argparse.ArgumentParser()
p.add_argument("--source", type=pathlib.Path, default=pathlib.Path.cwd())
p.add_argument("--out", required=True, type=pathlib.Path)
p.add_argument("--snapshot-only", action="store_true", help="prepare source without toolchain builds")
p.add_argument("--aws-acceptance", action="store_true", help="compile the explicitly opt-in AWS fault hook")
p.add_argument("--build-ca-file", type=pathlib.Path, help="optional reviewed public CA bundle for container dependency downloads")
a = p.parse_args()
source = a.source.resolve()
out = a.out.resolve()
if out == source or source in out.parents:
    raise SystemExit("artifact output must be outside the source tree")
out.mkdir(mode=0o700, parents=True, exist_ok=False)
build_ca_args = []
if a.build_ca_file:
    ca = a.build_ca_file.resolve()
    if not ca.is_file() or ca == source or source in ca.parents:
        raise SystemExit("build CA bundle must be a regular file outside source")
    with ca.open("rb") as file: ca_bytes = file.read(1024*1024+1)
    if len(ca_bytes) > 1024*1024 or b"PRIVATE KEY" in ca_bytes:
        raise SystemExit("bounded public certificates only; no private keys")
    ssl.create_default_context(cafile=str(ca))
    (out / "build-ca.pem").write_bytes(ca_bytes)
    build_ca_args = ["-v", str(out / "build-ca.pem") + ":/build-ca.pem:ro", "-e", "CARGO_HTTP_CAINFO=/build-ca.pem"]
preparation = Preparation(source, out)
run = preparation.run

def git(*args, env=None, data=None):
    return run(["git", "-c", "core.autocrlf=false", *args], env=env, data=data)

tracked = set(git("ls-files", "--cached", "-z").decode().split("\0")) - {""}
untracked = set(git("ls-files", "--others", "--exclude-standard", "-z").decode().split("\0")) - {""}
private_parts = {".git", ".aws", ".ssh", ".agents", ".codex", ".venv", "__pycache__", "node_modules", "target", "cdk.out", "artifacts", "operations"}
private_names = {"AGENTS.md", "WORKFLOW.md", "operator.json", "operation.json", "artifact.json", "capture.json", "known_hosts", "id_ed25519", "id_rsa", "credentials", "credentials.toml", "config.local.json"}
source_suffixes = {".rs", ".toml", ".lock", ".md", ".html", ".css", ".sh", ".py", ".yml", ".yaml", ".txt", ".snap", ".bin", ".selector", ".dat", ".cases", ".in", ".Dockerfile"}
new_json = {"crates/conformance/aws/runner.json", "crates/conformance/aws/toolchain.json", "crates/conformance/aws/operator.example.json", "crates/conformance/aws/tests/synthesized.json", "crates/conformance/aws/tests/legacy-direct-ssh-template.json", "crates/conformance/aws/tests/legacy-direct-ssh-deployed-template.json"}
public_pem = {"crates/net/testdata/localhost-cert.pem", "crates/net/testdata/localhost-key.pem", "crates/net/testdata/test-root-ca.pem"}

def excluded(name):
    path = pathlib.PurePosixPath(name)
    return (any(part in private_parts or part.startswith(".env") for part in path.parts)
        or path.name in private_names or (path.suffix in {".pem", ".key", ".p12", ".pfx"} and name not in public_pem))

files = []
for name in sorted(tracked | untracked):
    if excluded(name): continue
    path = source / name
    if not path.exists(): continue  # Working-tree deletions are part of this tree.
    if path.is_symlink() or not path.is_file(): raise RuntimeError("unsupported source entry: " + name)
    if name in untracked:
        if pathlib.PurePosixPath(name).suffix not in source_suffixes and name not in new_json:
            raise RuntimeError("unreviewed new source input: " + name)
    files.append(name)

snapshot = out / "source"
snapshot.mkdir()
with tempfile.TemporaryDirectory(prefix="ag4-git-") as temporary:
    temporary = pathlib.Path(temporary)
    objects = temporary / "objects"; objects.mkdir()
    original_objects = pathlib.Path(git("rev-parse", "--git-path", "objects").decode().strip())
    if not original_objects.is_absolute(): original_objects = source / original_objects
    env = dict(os.environ, GIT_INDEX_FILE=str(temporary / "index"), GIT_OBJECT_DIRECTORY=str(objects),
               GIT_ALTERNATE_OBJECT_DIRECTORIES=str(original_objects.resolve()))
    git("read-tree", "--empty", env=env)
    recorded = {}
    index_entries = []
    for name in files:
        path = source / name
        contents = path.read_bytes()
        mode = "100755" if path.stat().st_mode & stat.S_IXUSR else "100644"
        oid = git("hash-object", "-w", "--stdin", "--no-filters", env=env, data=contents).decode().strip()
        index_entries.append((mode + " " + oid + "\t" + name + "\0").encode())
        recorded[name] = (mode, hashlib.sha256(contents).digest())
    git("update-index", "-z", "--index-info", env=env, data=b"".join(index_entries))
    tree = git("write-tree", env=env).decode().strip()
    archive = temporary / "source.tar"
    git("archive", "--format=tar", "--output=" + str(archive), tree, env=env)
    run(["tar", "-xf", str(archive), "-C", str(snapshot)])
    exported = {str(path.relative_to(snapshot)) for path in snapshot.rglob("*") if path.is_file() or path.is_symlink()}
    if exported != set(recorded): raise RuntimeError("Git attributes changed the exported source set")
    for name, (mode, digest) in recorded.items():
        path = snapshot / name
        if path.is_symlink() or hashlib.sha256(path.read_bytes()).digest() != digest:
            raise RuntimeError("export changed source bytes: " + name)
        if ("100755" if path.stat().st_mode & stat.S_IXUSR else "100644") != mode:
            raise RuntimeError("export changed source mode: " + name)
        current = source / name
        if not current.is_file() or hashlib.sha256(current.read_bytes()).digest() != digest:
            raise RuntimeError("source changed during snapshot: " + name)
    current_names = set(git("ls-files", "--cached", "--others", "--exclude-standard", "-z").decode().split("\0")) - {""}
    current_names = {name for name in current_names if not excluded(name) and (source / name).exists()}
    if current_names != set(recorded): raise RuntimeError("source file set changed during snapshot")
    # Retain the exact tree as a standard Git bundle-independent archive.
    shutil.copyfile(archive, out / "source.tar")
(out / "source-tree.txt").write_text(tree + "\n")
for path in snapshot.rglob("*"):
    if path.is_file(): path.chmod(0o555 if path.stat().st_mode & stat.S_IXUSR else 0o444)
for path in sorted(snapshot.rglob("*"), reverse=True):
    if path.is_dir(): path.chmod(0o555)
snapshot.chmod(0o555)
if a.snapshot_only:
    preparation.check_cancelled()
    print(tree)
    raise SystemExit(0)

# Credential/configuration variables are not copied into the Linux build.
build_env = {k: v for k, v in os.environ.items() if not k.startswith(("AWS_", "BORROWSER_"))}
build_env.update(BORROWSER_AG4_SOURCE_TREE=tree, CARGO_TARGET_DIR=str(out / "native-target"))
if not run(["rustc", "--version"], env=build_env).decode().startswith("rustc 1.92.0 "):
    raise RuntimeError("Rust 1.92.0 is required")
features = ["--features", "aws-acceptance"] if a.aws_acceptance else []
run(["cargo", "build", "-p", "borrowser-conformance", "--release", "--locked", *features], cwd=snapshot, env=build_env)
native = out / "borrowser-conformance-controller"
shutil.copyfile(out / "native-target/release/borrowser-conformance", native); native.chmod(0o700)
image = "borrowser-ag4-builder:" + tree
run(["docker", "build", "--platform", "linux/amd64", "-t", image, "-f", str(snapshot / "tools/conformance/ag4-builder.Dockerfile"), str(snapshot / "tools/conformance")])
linux_target = out / "linux-target"; linux_target.mkdir()
preparation.container(["--platform", "linux/amd64", *build_ca_args, "-v", str(snapshot) + ":/source:ro", "-v", str(linux_target) + ":/target",
     "-e", "BORROWSER_AG4_SOURCE_TREE=" + tree, "-e", "CARGO_TARGET_DIR=/target"], [image,
     "cargo", "build", "-p", "borrowser-conformance", "--release", "--locked", *features])
linux = out / "borrowser-conformance-linux"
shutil.copyfile(linux_target / "release/borrowser-conformance", linux); linux.chmod(0o700)
identity = json.loads(preparation.container(["--platform", "linux/amd64", "--network=none", "-v", str(out) + ":/artifact:ro"], [image,
                          "/artifact/borrowser-conformance-linux", "--ag4-artifact-info"]))
controller = json.loads(run([str(native), "--ag4-artifact-info"]))
controller["platform"] = "linux-x86_64"
if identity != controller or identity["source_tree"] != tree:
    raise RuntimeError("prepared build identities differ")
preparation.check_cancelled()
(out / "artifact.json").write_text(json.dumps({"schema": 1, "sha256": hashlib.sha256(linux.read_bytes()).hexdigest(), "identity": identity}, indent=2) + "\n")
print(tree)
PY
