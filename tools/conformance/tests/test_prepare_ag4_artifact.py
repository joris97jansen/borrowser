import hashlib
import os
import json
from pathlib import Path
import subprocess
import signal
import sys
import tempfile
import unittest
import time

SCRIPT = Path(__file__).resolve().parents[1] / "prepare_ag4_artifact.sh"


class SourceSnapshot(unittest.TestCase):
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
