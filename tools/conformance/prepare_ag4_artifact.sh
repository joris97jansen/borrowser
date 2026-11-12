#!/bin/sh
# Standard Git temporary-index snapshot, then two builds from that exact tree.
set -eu
exec python3 - "$0" "$@" <<'PY'
import argparse, hashlib, json, os, pathlib, shutil, ssl, stat, sys, tarfile, tempfile
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

private_parts = {".git", ".aws", ".ssh", ".agents", ".codex", ".venv", "__pycache__", "node_modules", "target", "cdk.out", "artifacts", "operations"}
private_names = {"AGENTS.md", "WORKFLOW.md", "operator.json", "operation.json", "artifact.json", "capture.json", "known_hosts", "id_ed25519", "id_rsa", "credentials", "credentials.toml", "config.local.json"}
source_suffixes = {".rs", ".toml", ".lock", ".md", ".html", ".css", ".sh", ".py", ".yml", ".yaml", ".txt", ".snap", ".bin", ".selector", ".dat", ".cases", ".in", ".Dockerfile"}
new_json = {"crates/conformance/aws/runner.json", "crates/conformance/aws/toolchain.json", "crates/conformance/aws/operator.example.json", "crates/conformance/aws/tests/synthesized.json", "crates/conformance/aws/tests/legacy-direct-ssh-template.json", "crates/conformance/aws/tests/legacy-direct-ssh-deployed-template.json"}
public_pem = {"crates/net/testdata/localhost-cert.pem", "crates/net/testdata/localhost-key.pem", "crates/net/testdata/test-root-ca.pem"}

def excluded(name):
    path = pathlib.PurePosixPath(name)
    return (any(part in private_parts or part.startswith(".env") for part in path.parts)
        or path.name in private_names or (path.suffix in {".pem", ".key", ".p12", ".pfx"} and name not in public_pem))

def read_entry(root, name):
    # Never traverse a source-directory link or open a file through a link.
    # lstat/readlink also keep dangling links visible instead of treating them
    # as working-tree deletions. Git stores the target bytes, not target data.
    parts = pathlib.PurePosixPath(name).parts
    if not parts or parts[0] == "/" or ".." in parts:
        raise RuntimeError("unsafe source path: " + name)
    directory = os.open(root, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
    try:
        try:
            for part in parts[:-1]:
                child = os.open(part, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=directory)
                os.close(directory); directory = child
            before = os.stat(parts[-1], dir_fd=directory, follow_symlinks=False)
        except FileNotFoundError:
            return None  # An absent entry/parent is an intentional deletion.
        if stat.S_ISLNK(before.st_mode):
            contents = os.readlink(os.fsencode(parts[-1]), dir_fd=directory)
            mode = "120000"
        elif stat.S_ISREG(before.st_mode):
            fd = os.open(parts[-1], os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK, dir_fd=directory)
            with os.fdopen(fd, "rb") as file:
                opened = os.fstat(file.fileno())
                if not stat.S_ISREG(opened.st_mode) or (opened.st_dev, opened.st_ino) != (before.st_dev, before.st_ino):
                    raise RuntimeError("source changed during snapshot: " + name)
                contents = file.read()
            mode = "100755" if before.st_mode & stat.S_IXUSR else "100644"
        else:
            raise RuntimeError("unsupported source entry: " + name)
        after = os.stat(parts[-1], dir_fd=directory, follow_symlinks=False)
        if (before.st_dev, before.st_ino, before.st_mode, before.st_size, before.st_mtime_ns, before.st_ctime_ns) != (
                after.st_dev, after.st_ino, after.st_mode, after.st_size, after.st_mtime_ns, after.st_ctime_ns):
            raise RuntimeError("source changed during snapshot: " + name)
        return mode, contents
    finally:
        os.close(directory)


def validate_links(entries):
    directories = {str(parent) for name in entries for parent in pathlib.PurePosixPath(name).parents}
    for name, (mode, _contents) in entries.items():
        if mode != "120000": continue
        current, visited = name, set()
        while entries[current][0] == "120000":
            if current in visited:
                raise RuntimeError("cyclic source symlink: " + name)
            visited.add(current)
            target = os.fsdecode(entries[current][1])
            if not target or target.startswith("/"):
                raise RuntimeError("absolute or empty source symlink: " + name)
            parts = list(pathlib.PurePosixPath(current).parent.parts)
            for part in target.split("/"):
                # Every intermediate component must be an included directory;
                # do not normalize away a private path or traverse a file link.
                if "/".join(parts) in entries:
                    raise RuntimeError("source symlink traverses a file: " + name)
                if part == "..":
                    if not parts: raise RuntimeError("escaping source symlink: " + name)
                    parts.pop()
                elif part not in {"", "."}:
                    parts.append(part)
                candidate = "/".join(parts)
                if excluded(candidate):
                    raise RuntimeError("source symlink targets excluded input: " + name)
                if candidate not in entries and (candidate or ".") not in directories:
                    raise RuntimeError("dangling source symlink: " + name)
            current = "/".join(parts)
            if current not in entries:
                raise RuntimeError("source symlink must target an included file: " + name)


def source_entries():
    tracked = set(git("ls-files", "--cached", "-z").decode().split("\0")) - {""}
    untracked = set(git("ls-files", "--others", "--exclude-standard", "-z").decode().split("\0")) - {""}
    entries = {}
    for name in sorted(tracked | untracked):
        if excluded(name): continue
        entry = read_entry(source, name)
        if entry is None: continue
        if name in untracked:
            if entry[0] == "120000": raise RuntimeError("untracked source symlink: " + name)
            fixture_json = name.startswith("crates/conformance/fixtures/") and name.endswith((".fixture.json", ".expected.json"))
            if pathlib.PurePosixPath(name).suffix not in source_suffixes and name not in new_json and not fixture_json:
                raise RuntimeError("unreviewed new source input: " + name)
        entries[name] = entry
    validate_links(entries)
    return entries


entries = source_entries()

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
    index_entries = []
    for name, (mode, contents) in entries.items():
        oid = git("hash-object", "-w", "--stdin", "--no-filters", env=env, data=contents).decode().strip()
        index_entries.append((mode + " " + oid + "\t" + name + "\0").encode())
    git("update-index", "-z", "--index-info", env=env, data=b"".join(index_entries))
    tree = git("write-tree", env=env).decode().strip()
    archive = temporary / "source.tar"
    git("archive", "--format=tar", "--output=" + str(archive), tree, env=env)
    # Check the actual archive before extraction, including Git attributes and
    # link records. No archive link may introduce a different target/type.
    directories = {str(parent) for name in entries for parent in pathlib.PurePosixPath(name).parents} - {"."}
    with tarfile.open(archive) as bundle:
        seen, exported = set(), set()
        for member in bundle:
            name = member.name.rstrip("/") if member.isdir() else member.name
            if name in seen: raise RuntimeError("duplicate archive entry: " + name)
            seen.add(name)
            if member.isdir() and name in directories: continue
            if name not in entries: raise RuntimeError("unexpected archive entry: " + name)
            if member.issym():
                entry = "120000", os.fsencode(member.linkname)
            elif member.isfile():
                entry = ("100755" if member.mode & stat.S_IXUSR else "100644"), bundle.extractfile(member).read()
            else:
                raise RuntimeError("unsupported archive entry: " + name)
            if entry != entries[name]: raise RuntimeError("archive changed source entry: " + name)
            exported.add(name)
        if exported != set(entries): raise RuntimeError("Git attributes changed the exported source set")
    run(["tar", "-xf", str(archive), "-C", str(snapshot)])
    exported = set()
    for directory, subdirs, names in os.walk(snapshot, followlinks=False):
        for name in subdirs + names:
            path = pathlib.Path(directory) / name
            if not stat.S_ISDIR(path.lstat().st_mode): exported.add(str(path.relative_to(snapshot)))
    if exported != set(entries): raise RuntimeError("export changed source file set")
    for name, entry in entries.items():
        if read_entry(snapshot, name) != entry:
            raise RuntimeError("export changed source entry: " + name)
    # Retain the exact tree as a standard Git bundle-independent archive.
    shutil.copyfile(archive, out / "source.tar")
for name, (mode, _contents) in entries.items():
    if mode != "120000":
        os.chmod(snapshot / name, 0o555 if mode == "100755" else 0o444, follow_symlinks=False)
for name in sorted(directories, key=lambda name: len(pathlib.PurePosixPath(name).parts), reverse=True):
    os.chmod(snapshot / name, 0o555, follow_symlinks=False)
os.chmod(snapshot, 0o555, follow_symlinks=False)
if source_entries() != entries:
    raise RuntimeError("source changed during snapshot")
(out / "source-tree.txt").write_text(tree + "\n")
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
(out / "artifact.json").write_text(json.dumps({"schema": 2, "sha256": hashlib.sha256(linux.read_bytes()).hexdigest(), "identity": identity}, indent=2) + "\n")
print(tree)
PY
