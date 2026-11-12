import hashlib
import os
import json
from pathlib import Path
import subprocess
import signal
import stat
import sys
import tarfile
import tempfile
import unittest
import time

SCRIPT = Path(__file__).resolve().parents[1] / "prepare_ag4_artifact.sh"


class SourceSnapshot(unittest.TestCase):
    def snapshot(self, source, output, fault=None):
        env = dict(os.environ, PYTHONDONTWRITEBYTECODE="1")
        if fault:
            directory = output.parent / (output.name + "-fault"); directory.mkdir()
            (directory / "sitecustomize.py").write_text(fault)
            env["PYTHONPATH"] = str(directory)
        return subprocess.run(["sh", str(SCRIPT), "--source", str(source), "--out", str(output), "--snapshot-only"],
                              env=env, capture_output=True, timeout=60)

    def test_actual_eight_license_links_have_exact_git_index_archive_and_export_identity(self):
        repository = SCRIPT.parents[2]
        raw = subprocess.check_output(["git", "ls-tree", "-r", "-z", "HEAD", "--", "vendor/rust-url"], cwd=repository)
        links = {}
        for item in raw.split(b"\0"):
            if item.startswith(b"120000 "):
                header, name = item.split(b"\t", 1)
                target = subprocess.check_output(["git", "cat-file", "blob", header.split()[2]], cwd=repository)
                links[name.decode()] = target
        expected = {"vendor/rust-url/" + crate + "/LICENSE-" + license: ("../LICENSE-" + license).encode()
                    for crate in ("data-url", "idna", "percent_encoding", "url") for license in ("APACHE", "MIT")}
        self.assertEqual(links, expected)
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp); source, git = self.setup_repo(root)
            vendor = source / "vendor/rust-url"; vendor.mkdir(parents=True)
            for license in ("LICENSE-APACHE", "LICENSE-MIT"):
                data = subprocess.check_output(["git", "show", "HEAD:vendor/rust-url/" + license], cwd=repository)
                (vendor / license).write_bytes(data)
            for name, target in links.items():
                original = repository / name
                self.assertTrue(stat.S_ISLNK(original.lstat().st_mode))
                self.assertEqual(os.readlink(os.fsencode(original)), target)
                path = source / name; path.parent.mkdir(exist_ok=True)
                path.symlink_to(os.fsdecode(target))
            executable = source / "src/tool.sh"; executable.write_bytes(b"#!/bin/sh\nexit 0\n"); executable.chmod(0o755)
            git("add", "."); git("commit", "-m", "actual upstream license entries")
            before_index = (source / ".git/index").read_bytes()
            before_status = git("--no-optional-locks", "status", "--porcelain=v1", "-uall")
            expected_tree = git("rev-parse", "HEAD^{tree}").decode().strip()
            source_entries = {}
            for item in git("ls-tree", "-r", "-z", "HEAD").split(b"\0"):
                if item:
                    header, name = item.split(b"\t", 1)
                    source_entries[name.decode()] = header.split()[::2]  # mode, blob
            for attempt in range(2):
                output = root / ("snapshot-" + str(attempt)); result = self.snapshot(source, output)
                self.assertEqual(result.returncode, 0, result.stderr.decode())
                self.assertEqual((output / "source-tree.txt").read_text().strip(), expected_tree)
                # Independently reconstruct Git's index from the actual archive,
                # without using the snapshot implementation's entry collector.
                verifier = root / ("verify-" + str(attempt)); verifier.mkdir()
                def verify_git(*args, data=None):
                    return subprocess.check_output(["git", *args], cwd=verifier, input=data, stderr=subprocess.DEVNULL)
                verify_git("init"); verify_git("read-tree", "--empty")
                index = []
                with tarfile.open(output / "source.tar") as archive:
                    members = {member.name: member for member in archive if not member.isdir()}
                    self.assertEqual(set(members), set(source_entries))
                    for name, member in members.items():
                        mode = b"120000" if member.issym() else (b"100755" if member.mode & 0o100 else b"100644")
                        data = os.fsencode(member.linkname) if member.issym() else archive.extractfile(member).read()
                        blob = verify_git("hash-object", "-w", "--stdin", data=data).strip()
                        self.assertEqual([mode, blob], source_entries[name])
                        index.append(mode + b" " + blob + b"\t" + name.encode() + b"\0")
                    for name, target in links.items():
                        self.assertTrue(members[name].issym())
                        self.assertEqual(os.fsencode(members[name].linkname), target)
                        path = output / "source" / name
                        self.assertTrue(stat.S_ISLNK(path.lstat().st_mode))
                        self.assertEqual(os.readlink(os.fsencode(path)), target)
                        self.assertTrue(path.resolve(strict=True).is_relative_to((output / "source").resolve()))
                verify_git("update-index", "-z", "--index-info", data=b"".join(index))
                self.assertEqual(verify_git("write-tree").decode().strip(), expected_tree)
                extracted = root / ("extracted-" + str(attempt)); extracted.mkdir()
                subprocess.run(["tar", "-xf", str(output / "source.tar"), "-C", str(extracted)], check=True, timeout=10)
                for name, target in links.items():
                    self.assertTrue(stat.S_ISLNK((extracted / name).lstat().st_mode))
                    self.assertEqual(os.readlink(os.fsencode(extracted / name)), target)
                    self.assertEqual(os.readlink(os.fsencode(source / name)), target)
                self.assertEqual((output / "source/src/tool.sh").stat().st_mode & 0o777, 0o555)
                self.assertEqual(executable.stat().st_mode & 0o777, 0o755)
                self.assertEqual((source / ".git/index").read_bytes(), before_index)
                self.assertEqual(git("--no-optional-locks", "status", "--porcelain=v1", "-uall"), before_status)

    def test_tracked_file_link_chains_and_link_deletion(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp); source, git = self.setup_repo(root)
            (source / "src/first.rs").symlink_to("./second.rs")
            (source / "src/second.rs").symlink_to("main.rs")
            git("add", "."); git("commit", "-m", "file links")
            result = self.snapshot(source, root / "chain")
            self.assertEqual(result.returncode, 0, result.stderr.decode())
            self.assertEqual(os.readlink(root / "chain/source/src/first.rs"), "./second.rs")
            (source / "src/first.rs").unlink()
            result = self.snapshot(source, root / "deleted-link")
            self.assertEqual(result.returncode, 0, result.stderr.decode())
            self.assertFalse((root / "deleted-link/source/src/first.rs").exists())
            self.assertFalse((root / "deleted-link/source/src/first.rs").is_symlink())

    def test_invalid_tracked_links_reject_without_reading_or_chmodding_external_targets(self):
        targets = ("ABSOLUTE", "../../external.rs", "../.aws/credentials", "../operator.json", "../.env",
                   "../target/private.rs", "missing.rs", "../src", "link.rs", "other.rs",
                   "main.rs/../deleted.rs", "main.rs/", "../.aws/../src/main.rs")
        for target in targets:
            with self.subTest(target=target), tempfile.TemporaryDirectory() as temp:
                root = Path(temp); source, git = self.setup_repo(root)
                external = root / "external.rs"; external.write_bytes(b"PRIVATE SENTINEL"); external.chmod(0o600)
                # A FIFO makes an accidental dereference/read observable as a
                # hung preparation, without opening any real developer files.
                external_fifo = root / "external-fifo.rs"; os.mkfifo(external_fifo)
                (source / ".aws").mkdir(); (source / ".aws/credentials").symlink_to(external_fifo)
                (source / "operator.json").symlink_to(external)
                (source / ".env").symlink_to(external)
                (source / "target").mkdir(); (source / "target/private.rs").symlink_to(external_fifo)
                (source / "src/other.rs").symlink_to("link.rs")
                actual = str(external_fifo) if target == "ABSOLUTE" else target
                (source / "src/link.rs").symlink_to(actual)
                git("add", "src"); git("commit", "-m", "invalid source link")
                before = external.stat()
                result = self.snapshot(source, root / "out")
                self.assertNotEqual(result.returncode, 0)
                self.assertIn(b"source symlink", result.stderr)
                self.assertFalse((root / "out/source.tar").exists())
                self.assertEqual(external.read_bytes(), b"PRIVATE SENTINEL")
                self.assertEqual(external.stat().st_mode, before.st_mode)
                self.assertEqual(external.stat().st_mtime_ns, before.st_mtime_ns)

    def test_untracked_links_including_dangling_links_are_not_source_deletions(self):
        for target in ("main.rs", "missing.rs", "/nonexistent/outside.rs"):
            with self.subTest(target=target), tempfile.TemporaryDirectory() as temp:
                root = Path(temp); source, _ = self.setup_repo(root)
                (source / "src/new.rs").symlink_to(target)
                result = self.snapshot(source, root / "out")
                self.assertNotEqual(result.returncode, 0)
                self.assertIn(b"untracked source symlink", result.stderr)

    def test_missing_target_directory_links_and_nonregular_entries_are_rejected(self):
        for mode in ("deleted-target", "directory-link", "fifo"):
            with self.subTest(mode=mode), tempfile.TemporaryDirectory() as temp:
                root = Path(temp); source, git = self.setup_repo(root)
                (source / "src/link.rs").symlink_to("main.rs")
                git("add", "."); git("commit", "-m", "tracked link")
                if mode == "deleted-target": (source / "src/main.rs").unlink()
                elif mode == "fifo":
                    (source / "src/main.rs").unlink(); os.mkfifo(source / "src/main.rs")
                else:
                    (source / "src").rename(root / "external")
                    (source / "src").symlink_to(root / "external", target_is_directory=True)
                result = self.snapshot(source, root / "out")
                self.assertNotEqual(result.returncode, 0)
                self.assertFalse((root / "out/source.tar").exists())

    def test_symlinks_are_never_opened_for_data_or_chmodded(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp); source, git = self.setup_repo(root)
            (source / "src/link.rs").symlink_to("main.rs")
            git("add", "."); git("commit", "-m", "tracked link")
            fault = '''import os,stat,sys
def audit(event,args):
 if event=='open' and isinstance(args[0],str) and args[0].endswith('link.rs'):
  assert args[2] & os.O_NOFOLLOW, 'attempted symlink data read'
 if event=='os.chmod':
  assert not stat.S_ISLNK(os.lstat(args[0]).st_mode), 'attempted symlink chmod'
sys.addaudithook(audit)
'''
            result = self.snapshot(source, root / "out", fault)
            self.assertEqual(result.returncode, 0, result.stderr.decode())
            self.assertEqual((source / "src/main.rs").stat().st_mode & 0o777, 0o644)

    def test_source_mutations_after_capture_are_rejected(self):
        for mutation in ("link-target", "link-to-file", "link-deletion", "file-bytes", "file-mode"):
            with self.subTest(mutation=mutation), tempfile.TemporaryDirectory() as temp:
                root = Path(temp); source, git = self.setup_repo(root)
                link = source / "src/link.rs"; link.symlink_to("main.rs")
                git("add", "."); git("commit", "-m", "tracked link")
                changes = {
                    "link-target": "link.unlink(); link.symlink_to('deleted.rs')",
                    "link-to-file": "link.unlink(); link.write_text('main.rs')",
                    "link-deletion": "link.unlink()",
                    "file-bytes": "link.with_name('main.rs').write_text('changed bytes')",
                    "file-mode": "link.with_name('main.rs').chmod(0o755)",
                }
                fault = "import subprocess\nfrom pathlib import Path\noriginal=subprocess.Popen\nlink=Path(" + repr(str(link)) + ")\n"
                fault += "def spawn(args,*a,**kw):\n if args[0]=='git' and 'archive' in args:\n  " + changes[mutation] + "\n return original(args,*a,**kw)\nsubprocess.Popen=spawn\n"
                result = self.snapshot(source, root / "out", fault)
                self.assertNotEqual(result.returncode, 0)
                self.assertIn(b"source changed during snapshot", result.stderr)
                self.assertFalse((root / "out/source-tree.txt").exists())

    def test_link_mutation_during_readlink_is_rejected(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp); source, git = self.setup_repo(root)
            link = source / "src/link.rs"; link.symlink_to("main.rs")
            git("add", "."); git("commit", "-m", "tracked link")
            fault = "import os\nfrom pathlib import Path\nlink=Path(" + repr(str(link)) + ")\noriginal=os.readlink\n"
            fault += "def readlink(path,*a,**kw):\n value=original(path,*a,**kw)\n if path==b'link.rs':\n  link.unlink(); link.symlink_to('deleted.rs')\n return value\nos.readlink=readlink\n"
            result = self.snapshot(source, root / "out", fault)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn(b"source changed during snapshot", result.stderr)

    def test_export_ignored_link_target_is_rejected_before_tar_extraction(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp); source, git = self.setup_repo(root)
            (source / "src/link.rs").symlink_to("main.rs")
            (source / ".gitattributes").write_text("src/main.rs export-ignore\n")
            git("add", "."); git("commit", "-m", "excluded archive target")
            fault = "import subprocess\noriginal=subprocess.Popen\ndef spawn(args,*a,**kw):\n assert args[0]!='tar', 'unsafe archive extracted'\n return original(args,*a,**kw)\nsubprocess.Popen=spawn\n"
            result = self.snapshot(source, root / "out", fault)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn(b"Git attributes changed the exported source set", result.stderr)
            self.assertNotIn(b"unsafe archive extracted", result.stderr)

    def test_changed_archive_targets_types_bytes_modes_and_paths_never_reach_tar(self):
        for mutation in ("link-target", "hardlink", "file-bytes", "file-mode", "escaping-path"):
            with self.subTest(mutation=mutation), tempfile.TemporaryDirectory() as temp:
                root = Path(temp); source, git = self.setup_repo(root)
                (source / "src/link.rs").symlink_to("main.rs")
                git("add", "."); git("commit", "-m", "tracked link")
                sentinel = root / "outside.rs"; sentinel.write_bytes(b"UNCHANGED"); sentinel.chmod(0o600)
                fault = "import io,subprocess,tarfile\noriginal=subprocess.Popen\nmutation=" + repr(mutation) + "\noutside=" + repr(str(sentinel)) + "\n"
                fault += '''def spawn(args,*a,**kw):
 if args[0]=='tar': raise AssertionError('unsafe archive extracted')
 child=original(args,*a,**kw)
 if args[0]=='git' and 'archive' in args:
  path=next(x.split('=',1)[1] for x in args if x.startswith('--output='))
  original_wait=child.wait
  def wait(*a,**kw):
   result=original_wait(*a,**kw)
   with tarfile.open(path) as archive:
    members=[(m,archive.extractfile(m).read() if m.isfile() else None) for m in archive]
   with tarfile.open(path,'w') as archive:
    for member,data in members:
     if member.name=='src/link.rs':
      if mutation=='link-target': member.linkname=outside
      if mutation=='hardlink': member.type=tarfile.LNKTYPE; member.linkname=outside
     if member.name=='src/main.rs':
      if mutation=='file-bytes': data=b'changed'; member.size=len(data)
      if mutation=='file-mode': member.mode=0o755
     archive.addfile(member,io.BytesIO(data) if data is not None else None)
    if mutation=='escaping-path':
     member=tarfile.TarInfo('../outside.rs'); member.size=7
     archive.addfile(member,io.BytesIO(b'CHANGED'))
   return result
  child.wait=wait
 return child
subprocess.Popen=spawn
'''
                result = self.snapshot(source, root / "out", fault)
                self.assertNotEqual(result.returncode, 0)
                self.assertIn(b"archive", result.stderr)
                self.assertNotIn(b"unsafe archive extracted", result.stderr)
                self.assertEqual(sentinel.read_bytes(), b"UNCHANGED")
                self.assertEqual(sentinel.stat().st_mode & 0o777, 0o600)

    def test_regular_file_swapped_for_external_link_before_open_is_not_followed(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp); source, _ = self.setup_repo(root)
            external = root / "external"; os.mkfifo(external)
            fault = "import os\nfrom pathlib import Path\npath=Path(" + repr(str(source / "src/main.rs")) + ")\nexternal=" + repr(str(external)) + "\noriginal=os.open\n"
            fault += "def open_file(name,flags,*a,**kw):\n if name=='main.rs':\n  path.unlink(); path.symlink_to(external)\n return original(name,flags,*a,**kw)\nos.open=open_file\n"
            result = self.snapshot(source, root / "out", fault)
            self.assertNotEqual(result.returncode, 0)
            self.assertFalse((root / "out/source.tar").exists())
            self.assertTrue(stat.S_ISFIFO(external.lstat().st_mode))

    def test_source_link_change_during_read_only_permissions_is_rejected(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp); source, git = self.setup_repo(root)
            link = source / "src/link.rs"; link.symlink_to("main.rs")
            git("add", "."); git("commit", "-m", "tracked link")
            output = root / "out"
            fault = "import os\nfrom pathlib import Path\nlink=Path(" + repr(str(link)) + ")\nsnapshot=" + repr(str((output / "source").resolve())) + "\noriginal=os.chmod\nchanged=False\n"
            fault += "def chmod(path,*a,**kw):\n global changed\n if str(path).startswith(snapshot) and not changed:\n  link.unlink(); link.symlink_to('deleted.rs'); changed=True\n return original(path,*a,**kw)\nos.chmod=chmod\n"
            result = self.snapshot(source, output, fault)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn(b"source changed during snapshot", result.stderr)
            self.assertFalse((output / "source-tree.txt").exists())
            self.assertEqual(os.readlink(link), "deleted.rs", "late mutation hook must execute")

    def test_ag7_reference_and_request_changes_affect_snapshot_identity(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp); source, _ = self.setup_repo(root)
            fixtures = source / "crates/conformance/fixtures/external"
            raw = fixtures / "raw/css"; raw.mkdir(parents=True)
            test = raw / "test.html"; reference = raw / "ref.html"
            metadata = fixtures / "case.fixture.json"
            test.write_text('<!doctype html><link rel=match href=ref.html>')
            reference.write_text('<!doctype html><p id=probe>reference</p>')
            metadata.write_text('{"observations":{"target":"probe"}}')
            trees = []
            for index in range(3):
                output = root / ("ag7-snapshot-" + str(index))
                result = subprocess.run(["sh", str(SCRIPT), "--source", str(source), "--out", str(output), "--snapshot-only"], capture_output=True, timeout=30)
                self.assertEqual(result.returncode, 0, result.stderr.decode())
                for path in (test, reference, metadata):
                    self.assertEqual((output / "source" / path.relative_to(source)).read_bytes(), path.read_bytes())
                trees.append((output / "source-tree.txt").read_text())
                if index == 0: reference.write_text('<!doctype html><p id=probe>changed reference</p>')
                if index == 1: metadata.write_text('{"observations":{"target":"different"}}')
            self.assertEqual(len(set(trees)), 3)

    def test_ag6_fixture_json_is_snapshotted_and_changes_source_identity(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp); source, _ = self.setup_repo(root)
            fixtures = source / "crates/conformance/fixtures"; fixtures.mkdir(parents=True)
            metadata = fixtures / "case.fixture.json"
            expected = fixtures / "case.expected.json"
            metadata.write_text('{"id":"case/one"}\n')
            expected.write_text('{"value":1}\n')
            trees = []
            for index in range(3):
                output = root / ("snapshot-" + str(index))
                result = subprocess.run(["sh", str(SCRIPT), "--source", str(source), "--out", str(output), "--snapshot-only"], capture_output=True, timeout=30)
                self.assertEqual(result.returncode, 0, result.stderr.decode())
                for path in (metadata, expected):
                    self.assertEqual((output / "source" / path.relative_to(source)).read_bytes(), path.read_bytes())
                trees.append((output / "source-tree.txt").read_text())
                if index == 0: metadata.write_text('{"id":"case/two"}\n')
                if index == 1: expected.write_text('{"value":2}\n')
            self.assertEqual(len(set(trees)), 3)
            (fixtures / "unrelated.json").write_text("{}")
            result = subprocess.run(["sh", str(SCRIPT), "--source", str(source), "--out", str(root / "rejected"), "--snapshot-only"], capture_output=True, timeout=30)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn(b"unreviewed new source input", result.stderr)

    def setup_repo(self, root):
        source = root / "repo"; source.mkdir()
        def git(*args):
            return subprocess.check_output(["git", *args], cwd=source, stderr=subprocess.DEVNULL)
        git("init"); git("config", "user.email", "fixture@example.invalid"); git("config", "user.name", "AG4 fixture")
        (source / "src").mkdir()
        (source / "src/main.rs").write_text("old HEAD\n")
        (source / "src/deleted.rs").write_text("delete me\n")
        (source / ".gitignore").write_text("target/\n.env\n")
        git("add", "."); git("commit", "-m", "test fixture")
        return source, git

    def test_complete_working_snapshot_without_real_index_changes(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp); source, git = self.setup_repo(root)
            (source / "src/main.rs").write_text("staged\n"); git("add", "src/main.rs")
            (source / "src/main.rs").write_text("unstaged intended bytes\n")
            (source / "src/new.rs").write_text("new source\n")
            (source / "src/deleted.rs").unlink()
            (source / ".aws").mkdir(); (source / ".aws/credentials").write_text("SECRET")
            (source / "operator.json").write_text("SECRET")
            (source / ".env").write_text("SECRET")
            (source / "target").mkdir(); (source / "target/generated.rs").write_text("GENERATED")
            before_index = (source / ".git/index").read_bytes()
            before_status = git("status", "--porcelain=v1", "-uall")
            output = root / "snapshot"
            completed = subprocess.run(["sh", str(SCRIPT), "--source", str(source), "--out", str(output), "--snapshot-only"], capture_output=True, timeout=30)
            self.assertEqual(completed.returncode, 0, completed.stderr.decode())
            self.assertEqual((source / ".git/index").read_bytes(), before_index)
            self.assertEqual(git("status", "--porcelain=v1", "-uall"), before_status)
            self.assertEqual((output / "source/src/main.rs").read_text(), "unstaged intended bytes\n")
            self.assertTrue((output / "source/src/new.rs").exists())
            for name in ["src/deleted.rs", ".aws", "operator.json", ".env", "target"]:
                self.assertFalse((output / "source" / name).exists(), name)
            tree = (output / "source-tree.txt").read_text().strip()
            self.assertNotEqual(tree, git("rev-parse", "HEAD^{tree}").decode().strip())
            self.assertEqual(len(tree), 40)
            self.assertEqual((output / "source/src/main.rs").stat().st_mode & 0o222, 0)

    def test_unknown_new_input_and_symlink_are_rejected(self):
        for name in ["src/secret.json", "src/link.rs"]:
            with tempfile.TemporaryDirectory() as temp:
                root = Path(temp); source, _ = self.setup_repo(root)
                if name.endswith("link.rs"): (source / name).symlink_to("main.rs")
                else: (source / name).write_text("not reviewed")
                result = subprocess.run(["sh", str(SCRIPT), "--source", str(source), "--out", str(root / "out"), "--snapshot-only"], capture_output=True, timeout=30)
                self.assertNotEqual(result.returncode, 0)

    def test_explicit_build_trust_is_public_bounded_and_outside_snapshot(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp); source, _ = self.setup_repo(root)
            public = root / "public-ca.pem"
            public.write_bytes((Path(__file__).resolve().parents[3] / "crates/net/testdata/test-root-ca.pem").read_bytes())
            output = root / "public-output"
            result = subprocess.run(["sh", str(SCRIPT), "--source", str(source), "--out", str(output), "--snapshot-only", "--build-ca-file", str(public)], capture_output=True, timeout=30)
            self.assertEqual(result.returncode, 0, result.stderr.decode())
            self.assertEqual((output / "build-ca.pem").read_bytes(), public.read_bytes())
            self.assertFalse((output / "source/build-ca.pem").exists())
            for index, data in enumerate([b"-----BEGIN PRIVATE KEY-----", b"x" * (1024*1024+1), b"not a certificate"]):
                public.write_bytes(data)
                result = subprocess.run(["sh", str(SCRIPT), "--source", str(source), "--out", str(root / ("bad-" + str(index))), "--snapshot-only", "--build-ca-file", str(public)], capture_output=True, timeout=30)
                self.assertNotEqual(result.returncode, 0)

    def test_actual_preparation_reaps_builds_on_interrupt_exception_timeout_and_failure(self):
        for mode in ("SIGINT", "SIGTERM", "exception", "timeout", "overflow", "failure", "descendant"):
            with self.subTest(mode=mode), tempfile.TemporaryDirectory() as temp:
                root = Path(temp); source, _ = self.setup_repo(root)
                helpers = root / "helpers"; helpers.mkdir()
                marker = root / "pids"
                (helpers / "rustc").write_text("#!/bin/sh\nprintf 'rustc 1.92.0 fixture\\n'\n")
                child_code = "import signal,time; signal.signal(signal.SIGTERM,signal.SIG_IGN); time.sleep(300)"
                cargo = "#!" + sys.executable + "\nimport os,pathlib,subprocess,sys,time\n"
                cargo += "child=subprocess.Popen([sys.executable,'-c'," + repr(child_code) + "])\n"
                cargo += "pathlib.Path(" + repr(str(marker)) + ").write_text(str(os.getpid())+' '+str(child.pid))\n"
                if mode == "overflow": cargo += "sys.stdout.write('x'*5000000); sys.stdout.flush()\n"
                if mode == "failure": cargo += "sys.exit(7)\n"
                elif mode == "descendant": cargo += "sys.exit(0)\n"
                else: cargo += "time.sleep(300)\n"
                (helpers / "cargo").write_text(cargo)
                for file in (helpers / "rustc", helpers / "cargo"): file.chmod(0o700)
                env = dict(os.environ, PATH=str(helpers) + os.pathsep + os.environ["PATH"])
                # Fault injection is confined to the test interpreter, not a
                # production bypass or changed preparation timeout.
                if mode in ("exception", "timeout"):
                    fault = root / "fault"; fault.mkdir()
                    code = "import os,time\nmarker=" + repr(str(marker)) + "\n"
                    if mode == "exception":
                        code += "original=os.fstat\ndef fail(fd):\n if os.path.exists(marker):\n  os.fstat=original\n  raise OSError('injected polling exception')\n return original(fd)\nos.fstat=fail\n"
                    else:
                        code += "original=time.monotonic\ntime.monotonic=lambda: original()+(1801 if os.path.exists(marker) else 0)\n"
                    (fault / "sitecustomize.py").write_text(code)
                    env["PYTHONPATH"] = str(fault)
                with subprocess.Popen(["sh", str(SCRIPT), "--source", str(source), "--out", str(root / "out")],
                                      env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE) as preparation:
                    deadline = time.monotonic() + 10
                    while not marker.exists() and preparation.poll() is None and time.monotonic() < deadline: time.sleep(0.01)
                    self.assertTrue(marker.exists(), preparation.stderr.read() if preparation.poll() is not None else "helper did not start")
                    if mode.startswith("SIG"): preparation.send_signal(getattr(signal, mode))
                    _stdout, stderr = preparation.communicate(timeout=10)
                    self.assertNotEqual(preparation.returncode, 0, stderr.decode())
                    if mode == "exception": self.assertIn(b"injected polling exception", stderr)
                    if mode in ("timeout", "overflow"): self.assertIn(b"time/output limit", stderr)
                    for pid in marker.read_text().split():
                        observation = subprocess.run(["ps", "-p", pid, "-o", "stat="], capture_output=True, timeout=5)
                        self.assertTrue(not observation.stdout.strip() or observation.stdout.strip().startswith(b"Z"), "build still running: " + pid)

    @unittest.skipUnless(os.environ.get("BORROWSER_AG4_DOCKER_TEST_IMAGE"), "opt-in real Docker cancellation")
    def test_actual_preparation_stops_its_real_container(self):
        import shutil
        docker = shutil.which("docker")
        self.assertIsNotNone(docker)
        image = os.environ["BORROWSER_AG4_DOCKER_TEST_IMAGE"]
        self.assertIn("@sha256:", image, "reviewed pinned image required")
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp); source, _ = self.setup_repo(root)
            helpers = root / "helpers"; helpers.mkdir()
            (helpers / "rustc").write_text("#!/bin/sh\nprintf 'rustc 1.92.0 fixture\\n'")
            (helpers / "cargo").write_text("#!" + sys.executable + "\nimport os,pathlib\np=pathlib.Path(os.environ['CARGO_TARGET_DIR'])/'release/borrowser-conformance'\np.parent.mkdir(parents=True)\np.write_text('fixture')\n")
            # Native build is a controlled helper. Container creation, execution,
            # inspection, cancellation and removal use the real Docker daemon.
            (helpers / "docker").write_text("#!" + sys.executable + "\n" + '''
import os,sys
args=sys.argv[1:]
if args[0]=='build': sys.exit(0)
if args[0]=='create':
 index=next(i for i,a in enumerate(args) if a.startswith('borrowser-ag4-builder:'))
 args=args[:index]+[''' + repr(image) + ''','sh','-c','echo started >/target/container-started; trap "" TERM; sleep 300']
os.execv(''' + repr(docker) + ''',['docker']+args)
''')
            for file in helpers.iterdir(): file.chmod(0o700)
            output = root / "out"
            with subprocess.Popen(["sh", str(SCRIPT), "--source", str(source), "--out", str(output)],
                env=dict(os.environ, PATH=str(helpers) + os.pathsep + os.environ["PATH"]), stdout=subprocess.PIPE, stderr=subprocess.PIPE) as preparation:
                deadline = time.monotonic() + 30
                while not (output / "linux-target/container-started").exists() and preparation.poll() is None and time.monotonic() < deadline: time.sleep(0.05)
                self.assertTrue((output / "linux-target/container-started").exists())
                identity = json.loads((output / "preparation-container.json").read_text())
                preparation.send_signal(signal.SIGINT)
                _, stderr = preparation.communicate(timeout=40)
                self.assertNotEqual(preparation.returncode, 0, stderr.decode())
                self.assertFalse((output / "preparation-container.json").exists(), stderr.decode())
                result = subprocess.run([docker, "container", "ls", "--all", "--no-trunc", "--filter", "id="+identity["id"], "--format", "{{.ID}}"], capture_output=True, timeout=10)
                self.assertEqual(result.returncode, 0, result.stderr.decode())
                self.assertEqual(result.stdout.strip(), b"")

    def test_container_failure_and_ownership_boundaries_with_scripted_docker(self):
        for mode in ("success", "start-failure", "lost-create-response", "conflict", "remove-failure", "ambiguous-create"):
            with self.subTest(mode=mode), tempfile.TemporaryDirectory() as temp:
                root = Path(temp); helpers = root / "helpers"; helpers.mkdir()
                out = root / "out"; out.mkdir()
                state = root / "docker-state.json"
                cli = helpers / "docker"
                cli.write_text("#!" + sys.executable + "\nimport json,pathlib,sys\nmode=" + repr(mode) + "\nstate=pathlib.Path(" + repr(str(state)) + ")\n" + '''
args=sys.argv[1:]; cid='a'*64
if args[0]=='create':
 name=args[args.index('--name')+1]; token=args[args.index('--label')+1].split('=',1)[1]
 if mode!='ambiguous-create': state.write_text(json.dumps({'Id':cid,'Name':'/'+name,'Config':{'Labels':{'borrowser.ag4-preparation':token}}}))
 if mode in {'lost-create-response','ambiguous-create'}: sys.stderr.write('create response lost'); sys.exit(7)
 print(cid)
elif args[0]=='start':
 if mode=='start-failure': sys.stderr.write('build failed'); sys.exit(7)
 print('built')
elif args[:2]==['container','ls']:
 if state.exists(): print(cid)
elif args[0]=='inspect':
 value=json.loads(state.read_text())
 if mode=='conflict': value['Config']['Labels']['borrowser.ag4-preparation']='unrelated'
 print(json.dumps(value))
elif args[0]=='rm':
 if mode=='remove-failure': sys.stderr.write('remove failed'); sys.exit(7)
 state.unlink(); print(cid)
else: sys.exit(8)
''')
                cli.chmod(0o700)
                driver = "import sys; from pathlib import Path; sys.path.insert(0,sys.argv[1]); from prepare_ag4_process import Preparation; p=Preparation(Path(sys.argv[2]),Path(sys.argv[3])); p.container([],['synthetic-image','build'])"
                result = subprocess.run([sys.executable, "-c", driver, str(SCRIPT.parent), str(root), str(out)],
                    env=dict(os.environ, PATH=str(helpers) + os.pathsep + os.environ["PATH"]), capture_output=True, timeout=15)
                self.assertEqual(result.returncode == 0, mode == "success", result.stderr.decode())
                unresolved = mode in {"conflict", "remove-failure", "ambiguous-create"}
                self.assertEqual((out / "preparation-container.json").exists(), unresolved)
                self.assertEqual(state.exists(), mode in {"conflict", "remove-failure"})
                if mode == "start-failure": self.assertIn(b"build failed", result.stderr)
                if mode == "lost-create-response": self.assertIn(b"create response lost", result.stderr)
                if unresolved: self.assertIn(b"container cleanup:", result.stderr)


if __name__ == "__main__": unittest.main()
