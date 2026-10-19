"""Credential-free shell/diagnostic checks; helpers do not qualify a real guest."""
import base64
import gzip
from contextlib import contextmanager
import json
import os
from pathlib import Path
import re
import shutil
import selectors
import socket
import subprocess
import sys
import tempfile
import threading
import time
import unittest

ROOT = Path(__file__).resolve().parents[1]
RUN = "0123456789abcdef0123456789abcdef"
PHASES = {"setup": 30, "apt_update": 180, "apt_install": 420,
          "chromium_download": 180, "archive": 45, "sandbox": 15,
          "ssh_prepare": 10, "ssh_config": 5, "ssh_validate": 5, "ssh_restart": 10,
          "ssh_probe_loopback": 5, "ssh_probe_private": 10, "ready": 5}
SSH_POLICY = """PasswordAuthentication no
KbdInteractiveAuthentication no
PermitRootLogin no
AllowUsers ag4
AllowAgentForwarding no
AllowTcpForwarding no
X11Forwarding no
"""


def default_routes():
    return [{"dst": "default", "gateway": "10.77.0.1", "dev": "ens5"}]


def default_links():
    return [{"ifindex": 2, "ifname": "ens5", "flags": ["BROADCAST", "MULTICAST", "UP", "LOWER_UP"],
             "addr_info": [{"family": "inet", "local": "10.77.0.30", "prefixlen": 26, "scope": "global"}]}]


@contextmanager
def banner_listener(banner, port=0, absent=False, host="127.0.0.1"):
    """Test-only loopback server; bind before launch, EOF/wakeup synchronize exit."""
    with socket.socket() as listener:
        # Keep the absent port reserved without listening, avoiding a free-port race.
        listener.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        listener.bind((host, port))
        if absent:
            yield listener.getsockname()[1]
            return
        listener.listen(1)
        wake, stop = socket.socketpair()
        errors = []
        def serve():
            try:
                with selectors.DefaultSelector() as selector:
                    selector.register(listener, selectors.EVENT_READ)
                    selector.register(wake, selectors.EVENT_READ)
                    while True:
                        events = selector.select(10)
                        if not events:
                            raise TimeoutError("test listener owner did not finish")
                        for key, _ in events:
                            if key.fileobj is wake:
                                return
                            connection, peer = listener.accept()
                            with connection:
                                assert peer[0] == host
                                connection.settimeout(6)
                                if banner is not None:
                                    connection.sendall(banner)
                                try:
                                    assert connection.recv(1) == b"", "probe must not authenticate/send data"
                                except ConnectionResetError:
                                    pass  # bounded rejection may leave oversized server bytes unread
            except BaseException as error:
                errors.append(error)
        thread = threading.Thread(target=serve)
        thread.start()
        try:
            yield listener.getsockname()[1]
        finally:
            stop.sendall(b"x")
            thread.join(7)
            wake.close(); stop.close()
            assert not thread.is_alive(), "test listener cleanup exceeded bound"
            if errors:
                raise errors[0]


class Bootstrap(unittest.TestCase):
    def test_deadlines_compose_with_readiness_work_and_guest_safeguard(self):
        script = (ROOT / "bootstrap.sh").read_text()
        phases = dict((name, int(seconds)) for name, seconds in
                      re.findall(r"^run_phase (\w+) (\d+)$", script, re.M))
        self.assertEqual(phases, PHASES)
        self.assertEqual(len(re.findall(r"^run_phase ", script, re.M)), len(PHASES))
        self.assertIn('timeout --kill-after=5 "$seconds" /bin/bash -euo pipefail -c "$phase"', script)
        self.assertEqual(script.count("timeout --kill-after="), 1)
        # The curl retry loop is inside the whole-phase timeout, not 180s per retry.
        self.assertIn("--max-time 180 --retry 2", script)
        self.assertIn("deadline = time.monotonic() + 4", script)
        self.assertEqual(phases["ssh_probe_loopback"], 5)
        self.assertEqual(phases["ssh_probe_private"], 10)
        rust = (ROOT.parent / "src/aws/ssh.rs").read_text()
        readiness = int(re.search(r"const READINESS_SECONDS: u64 = (\d+);", rust)[1])
        reserve = int(re.search(r"const CAPTURE_RESERVE_SECONDS: u64 = (\d+);", rust)[1])
        inner = sum(phases.values()) + 5 * len(phases)
        self.assertEqual(inner, 985)
        self.assertEqual(readiness - inner, 95)
        self.assertEqual(reserve, 300 + 20 + 120 + 60 + 40)
        self.assertEqual(1800 - readiness - reserve, 180)
        runner = json.loads((ROOT / "runner.json").read_text())
        self.assertLess(readiness + reserve, 1800)
        self.assertLess(1800, runner["guest_stop_minutes"] * 60)
        self.assertIn("Duration::from_secs(1800)", (ROOT.parent / "src/aws/mod.rs").read_text())

    def execute(self, failure="", status=0, real_timeout=False, runtime_state="absent", probe_port=None,
                private_probe_port=None, routes=None, links=None):
        with tempfile.TemporaryDirectory() as directory:
            work = Path(directory)
            guest = work / "guest"
            commands = work / "bin"; commands.mkdir()
            for name in ["etc/apt/sources.list.d", "etc/apparmor.d", "etc/ssh/sshd_config.d",
                         "home/ag4/.ssh", "opt/borrowser-ag4", "tmp", "dev", "run",
                         "sys/kernel/security/apparmor"]:
                (guest / name).mkdir(parents=True)
            (guest / "sys/kernel/security/apparmor/profiles").write_text("borrowser-ag2-chromium (unconfined)\n")
            (guest / "etc/ssh/ssh_host_ed25519_key.pub").write_text("ssh-ed25519 TESTPUBLIC root@guest\n")
            runtime = guest / "run/sshd"
            if runtime_state == "symlink":
                target = work / "runtime-target"; target.mkdir(mode=0o755)
                runtime.symlink_to(target)
            elif runtime_state == "dangling-symlink":
                runtime.symlink_to(work / "missing-target")
            elif runtime_state == "file":
                runtime.write_bytes(b"must not replace")
            elif runtime_state != "absent":
                runtime.mkdir(mode=0o700 if runtime_state == "mode" else 0o755)
                if runtime_state == "contents":
                    (runtime / ".unexpected").write_bytes(b"must not erase")
            def runtime_identity():
                info = runtime.lstat()
                return (info.st_ino, info.st_mode, info.st_uid, info.st_gid,
                        os.readlink(runtime) if runtime.is_symlink() else None,
                        runtime.read_bytes() if runtime.is_file() else None,
                        [(p.name, p.read_bytes()) for p in runtime.iterdir()]
                        if runtime.is_dir() and not runtime.is_symlink() else None)
            previous_runtime = runtime_identity() if runtime_state != "absent" else None
            # Only test copies have their filesystem paths redirected. Production
            # has no test flag, environment bypass or alternative guest behavior.
            script = (ROOT / "bootstrap.sh").read_text()
            script = re.sub(r"/(etc|home|opt|tmp|dev|sys|run)/",
                            lambda match: str(guest / match[1]) + "/", script)
            script = script.replace("/usr/bin/python3 -I -", "python3 -I -")
            for name, value in {"RUN_ID": RUN, "GUEST_STOP_MINUTES": "45",
                "UBUNTU_SNAPSHOT": "20261013T000000Z", "CHROME_URL": "https://example.invalid/pinned.zip",
                "CHROME_SHA256": "0" * 64, "SSH_KEY_B64": base64.b64encode(b"ssh-ed25519 TESTPUBLIC").decode(),
                "CAPTURE_WRAPPER_GZIP_B64": base64.b64encode(gzip.compress((ROOT / "guest_capture.py").read_bytes(), mtime=0)).decode()}.items():
                script = script.replace("@@" + name + "@@", value)
            self.assertNotIn("@@", script)
            path = work / "bootstrap.sh"; path.write_text(script)
            helper = commands / "helper"
            helper.write_text("#!" + sys.executable + "\n" + '''
import json, os, pathlib, subprocess, sys, time
name = pathlib.Path(sys.argv[0]).name
args = sys.argv[1:]
with open(os.environ["HELPER_LOG"], "a") as file: file.write(json.dumps([name, *args]) + "\\n")
if name == "timeout":
    phase = args[-1]
    if os.environ.get("REAL_TIMEOUT") and phase == "apt_install":
        # Exercise actual GNU timeout status propagation using a short sleeping
        # helper. Production numeric bounds are checked separately, unchanged.
        result = subprocess.run([os.environ["REAL_TIMEOUT"], "--kill-after=1", "0.05", sys.executable, "-c", "import time; time.sleep(5)"])
        sys.exit(result.returncode)
    os.execv(args[2], args[2:])
phase = {"systemd-run":"setup", "curl":"chromium_download", "sha256sum":"archive",
         "apparmor_parser":"sandbox", "useradd":"ssh_prepare", "chown":"ssh_config",
         "sshd":"ssh_validate", "ip":"ssh_probe_private", "cut":"ready"}.get(name)
if name == "apt-get": phase = "apt_install" if "install" in args else "apt_update"
if name == "systemctl" and args == ["restart", "ssh"]: phase = "ssh_restart"
if name == "python3":
    phase = "ssh_probe_loopback" if args == ["-I", "-", "127.0.0.1"] else "ssh_probe_private"
if phase == os.environ.get("FAIL_PHASE"):
    print("uncontrolled package/command diagnostic", file=sys.stderr)
    sys.exit(int(os.environ["FAIL_STATUS"]))
if name == "ip":
    assert args in (["-j", "-4", "route", "show", "default"], ["-j", "-4", "address", "show", "scope", "global"])
    print(os.environ["TEST_ROUTES" if args[2] == "route" else "TEST_LINKS"])
if name == "python3":
    assert args[:2] == ["-I", "-"]
    if len(args) == 4:
        sys.argv = args[1:]
        exec(compile(sys.stdin.read(), "generated-private-address", "exec"))
    else:
        assert len(args) == 3
        variable = "PROBE_PORT" if args[2] == "127.0.0.1" else "PRIVATE_PROBE_PORT"
        if variable in os.environ:
            import socket
            original = socket.socket
            class ProbeSocket(original):
                def connect(self, address):
                    assert address == (args[2], 22)
                    return super().connect(("127.0.0.1", int(os.environ[variable])))
            socket.socket = ProbeSocket
            sys.argv = args[1:]
            exec(compile(sys.stdin.read(), "generated-ssh-probe", "exec"))
if name == "cut": print("ssh-ed25519 TESTPUBLIC")
if name == "install":
    assert args[:2] == ["-d", "-m"]
    mode = int(args[2], 8)
    paths = args[3:]
    if paths[:4] in (["-o", "ag4", "-g", "ag4"], ["-o", "root", "-g", "root"]): paths = paths[4:]
    for path in paths:
        pathlib.Path(path).mkdir(parents=True, exist_ok=True)
        os.chmod(path, mode)
if name == "chmod" and args[0] != "-R": os.chmod(args[1], int(args[0], 8))
if name == "stat":
    assert args[:2] == ["-c", "%u:%g:%a"] and args[2] == os.environ["RUNTIME_PATH"]
    # Native macOS helpers cannot chown to root; real uid/gid checks run in
    # the disposable Noble test. Mode and path state are actual filesystem data.
    uid = 1 if os.environ["RUNTIME_STATE"] == "owner" else 0
    gid = 1 if os.environ["RUNTIME_STATE"] == "group" else 0
    print(f"{uid}:{gid}:{os.lstat(args[2]).st_mode & 0o7777:o}")
if name == "find":
    assert args == [os.environ["RUNTIME_PATH"], "-mindepth", "1", "-maxdepth", "1", "-printf", "x", "-quit"]
    with os.scandir(args[0]) as entries:
        if next(entries, None) is not None: print("x", end="")
sys.exit(0)
''')
            helper.chmod(0o700)
            for name in ["timeout", "systemd-run", "systemctl", "apt-get", "curl", "sha256sum",
                         "unzip", "chmod", "rm", "apparmor_parser", "useradd", "install", "chown", "sshd", "python3", "ip", "cut", "stat", "find"]:
                (commands / name).symlink_to(helper)
            env = {"PATH": str(commands) + ":/usr/bin:/bin", "HELPER_LOG": str(work / "commands.jsonl"),
                   "FAIL_PHASE": failure, "FAIL_STATUS": str(status),
                   "RUNTIME_PATH": str(runtime), "RUNTIME_STATE": runtime_state,
                   "TEST_ROUTES": json.dumps(default_routes() if routes is None else routes) if not isinstance(routes, str) else routes,
                   "TEST_LINKS": json.dumps(default_links() if links is None else links) if not isinstance(links, str) else links}
            if probe_port is not None:
                env["PROBE_PORT"] = str(probe_port)
            if private_probe_port is not None:
                env["PRIVATE_PROBE_PORT"] = str(private_probe_port)
            if real_timeout:
                tool = shutil.which("timeout") or shutil.which("gtimeout")
                if not tool:
                    self.skipTest("GNU timeout unavailable; scripted status tests still run")
                env["REAL_TIMEOUT"] = tool
            result = subprocess.run(["/bin/bash", str(path)], env=env, capture_output=True, timeout=15)
            console = (guest / "dev/console").read_text()
            calls = [json.loads(line) for line in (work / "commands.jsonl").read_text().splitlines()]
            if previous_runtime is not None:
                self.assertEqual(runtime_identity(), previous_runtime)
                self.assertFalse(any(c[0] == "install" and str(runtime) in c for c in calls))
            if result.returncode == 0:
                self.assertFalse(runtime.is_symlink())
                self.assertTrue(runtime.is_dir())
                self.assertEqual(runtime.stat().st_mode & 0o7777, 0o755)
                self.assertEqual(list(runtime.iterdir()), [])
                if runtime_state == "absent":
                    self.assertIn(["install", "-d", "-m", "0755", "-o", "root", "-g", "root", str(runtime)], calls)
                self.assertEqual((guest / "etc/ssh/sshd_config.d/00-borrowser-ag4.conf").read_text(), SSH_POLICY)
                self.assertEqual((guest / "home/ag4/.ssh/authorized_keys").read_bytes(), b"ssh-ed25519 TESTPUBLIC")
                self.assertEqual((guest / "home/ag4/.ssh/authorized_keys").stat().st_mode & 0o777, 0o600)
                for directory in ["home/ag4/.ssh", "home/ag4/run", "tmp/ag4"]:
                    self.assertEqual((guest / directory).stat().st_mode & 0o777, 0o700)
                self.assertIn(["chown", "ag4:ag4", str(guest / "home/ag4/.ssh/authorized_keys")], calls)
                self.assertEqual((guest / "opt/borrowser-ag4/guest_capture.py").read_bytes(), (ROOT / "guest_capture.py").read_bytes())
                self.assertEqual((guest / "opt/borrowser-ag4/guest_capture.py").stat().st_mode & 0o777, 0o644)
            return result, console, calls

    def test_success_keeps_the_existing_readiness_authority(self):
        result, console, calls = self.execute()
        self.assertEqual(result.returncode, 0, result.stderr.decode())
        self.assertEqual(console, "BORROWSER_AG4_READY " + RUN + " ssh-ed25519 TESTPUBLIC\n")
        self.assertEqual([(c[-1], int(c[2])) for c in calls if c[0] == "timeout"], list(PHASES.items()))
        self.assertEqual([c for c in calls if c[0] == "sshd"], [["sshd", "-t"]])
        self.assertEqual([c for c in calls if c[:2] == ["systemctl", "restart"]], [["systemctl", "restart", "ssh"]])
        self.assertLess(calls.index(["sshd", "-t"]), calls.index(["systemctl", "restart", "ssh"]))
        self.assertLess(calls.index(["systemctl", "restart", "ssh"]), calls.index(["python3", "-I", "-", "127.0.0.1"]))
        self.assertLess(calls.index(["python3", "-I", "-", "127.0.0.1"]), calls.index(["python3", "-I", "-", "10.77.0.30"]))
        self.assertLess(calls.index(["python3", "-I", "-", "10.77.0.30"]), next(i for i, c in enumerate(calls) if c[0] == "cut"))

    def test_loopback_banner_is_required_before_ready(self):
        for name, banner, absent, valid in [
            ("absent", None, True, False), ("silent", None, False, False),
            ("valid", b"SSH-2.0-OpenSSH_9.6p1 Ubuntu\r\n", False, True),
            ("valid-maximum", b"SSH-2.0-" + b"x" * 245 + b"\r\n", False, True),
            ("wrong", b"HTTP/1.1 200 OK\r\n", False, False),
            ("malformed-ending", b"SSH-2.0-test\n", False, False),
            ("missing-version", b"SSH-2.0-\r\n", False, False),
            ("control", b"SSH-2.0-test\x00bad\r\n", False, False),
            ("oversized", b"SSH-2.0-" + b"x" * 255 + b"\r\n", False, False),
        ]:
            with self.subTest(case=name), banner_listener(banner, absent=absent) as port:
                result, console, calls = self.execute(probe_port=port)
                self.assertIn(["sshd", "-t"], calls)
                self.assertIn(["systemctl", "restart", "ssh"], calls)
                if valid:
                    self.assertEqual(result.returncode, 0, result.stderr.decode())
                    self.assertEqual(console, f"BORROWSER_AG4_READY {RUN} ssh-ed25519 TESTPUBLIC\n")
                else:
                    self.assertEqual(result.returncode, 1, result.stderr.decode())
                    self.assertEqual(console.strip(), f"BORROWSER_AG4_BOOTSTRAP_FAILED {RUN} ssh_probe_loopback 1")
                    self.assertNotIn("BORROWSER_AG4_READY", console)
                    self.assertEqual([c[-1] for c in calls if c[0] == "timeout"], list(PHASES)[:-2])

    def test_failures_preserve_phase_and_status_without_publishing_readiness(self):
        for phase, status in [("setup", 1), ("apt_update", 100), ("apt_install", 100),
                              ("chromium_download", 22), ("archive", 1), ("sandbox", 42),
                              ("ssh_prepare", 9), ("ssh_config", 1), ("ssh_validate", 255),
                              ("ssh_restart", 5), ("ssh_probe_loopback", 1), ("ssh_probe_private", 2), ("ready", 1), ("apt_install", 124), ("apt_install", 137)]:
            with self.subTest(phase=phase, status=status):
                result, console, calls = self.execute(phase, status)
                self.assertEqual(result.returncode, status)
                self.assertEqual(console.strip(), f"BORROWSER_AG4_BOOTSTRAP_FAILED {RUN} {phase} {status}")
                self.assertLess(len(console.encode()), 128)
                self.assertNotIn("uncontrolled", console)
                self.assertNotIn("BORROWSER_AG4_READY", console)
                started = [c[-1] for c in calls if c[0] == "timeout"]
                self.assertEqual(started, list(PHASES)[:list(PHASES).index(phase) + 1])

    def test_private_banner_is_required_after_loopback_success(self):
        for name, banner, absent, valid in [
            ("valid", b"SSH-2.0-OpenSSH_9.6p1 Ubuntu\r\n", False, True),
            ("refused", None, True, False), ("silent", None, False, False),
            ("wrong", b"HTTP/1.1 200 OK\r\n", False, False),
            ("malformed", b"SSH-2.0-test\n", False, False),
            ("control", b"SSH-2.0-test\x00bad\r\n", False, False),
            ("oversized", b"SSH-2.0-" + b"x" * 255 + b"\r\n", False, False),
        ]:
            with self.subTest(case=name), banner_listener(b"SSH-2.0-loopback\r\n") as loopback:
                with banner_listener(banner, absent=absent) as private:
                    result, console, calls = self.execute(probe_port=loopback, private_probe_port=private)
                self.assertIn(["python3", "-I", "-", "127.0.0.1"], calls)
                self.assertIn(["python3", "-I", "-", "10.77.0.30"], calls)
                if valid:
                    self.assertEqual(result.returncode, 0, result.stderr.decode())
                    self.assertEqual(console, f"BORROWSER_AG4_READY {RUN} ssh-ed25519 TESTPUBLIC\n")
                else:
                    self.assertEqual(result.returncode, 1, result.stderr.decode())
                    self.assertEqual(console.strip(), f"BORROWSER_AG4_BOOTSTRAP_FAILED {RUN} ssh_probe_private 1")
                    self.assertFalse(any(c[0] == "cut" for c in calls))

    def test_private_address_discovery_rejects_incomplete_or_ambiguous_state(self):
        cases = [("no-route", [], default_links()),
                 ("multiple-routes", default_routes() * 2, default_links()),
                 ("malformed-route", "not json", default_links()),
                 ("oversized-route", " " * 8193, default_links()),
                 ("no-link", default_routes(), []),
                 ("duplicate-link", default_routes(), default_links() * 2),
                 ("malformed-link", default_routes(), [None]),
                 ("oversized-links", default_routes(), " " * 8193)]
        for field, value in [("dev", "lo"), ("dev", "ens5;false"), ("dev", "missing"),
                             ("dst", "10.0.0.0/8"), ("type", "blackhole"), ("nexthops", [])]:
            route = default_routes(); route[0][field] = value
            cases.append(("route-" + field, route, default_links()))
        for field, value in [("ifindex", True), ("flags", []), ("addr_info", []),
                             ("addr_info", default_links()[0]["addr_info"] * 2)]:
            links = default_links(); links[0][field] = value
            cases.append(("link-" + field, default_routes(), links))
        for field, value in [("local", "not-ipv4"), ("local", "::1"), ("local", "127.0.0.1"),
                             ("local", "169.254.169.254"), ("local", "8.8.8.8"),
                             ("family", "inet6"), ("scope", "host"), ("secondary", True),
                             ("prefixlen", 33), ("prefixlen", "26")]:
            links = default_links(); links[0]["addr_info"][0][field] = value
            cases.append(("address-" + field, default_routes(), links))
        for name, routes, links in cases:
            with self.subTest(case=name):
                result, console, calls = self.execute(routes=routes, links=links)
                self.assertEqual(result.returncode, 1, result.stderr.decode())
                self.assertEqual(console.strip(), f"BORROWSER_AG4_BOOTSTRAP_FAILED {RUN} ssh_probe_private 1")
                self.assertIn(b"primary private IPv4 unavailable or ambiguous", result.stderr)
                self.assertNotIn(["python3", "-I", "-", "10.77.0.30"], calls)
                self.assertFalse(any(c[0] == "cut" for c in calls))

    def test_real_timeout_status_reaches_the_console_marker(self):
        result, console, _ = self.execute(real_timeout=True)
        self.assertEqual(result.returncode, 124)
        self.assertEqual(console.strip(), f"BORROWSER_AG4_BOOTSTRAP_FAILED {RUN} apt_install 124")

    def test_runtime_directory_is_reused_only_when_valid_and_never_repaired(self):
        result, console, _ = self.execute(runtime_state="valid")
        self.assertEqual(result.returncode, 0, result.stderr.decode())
        self.assertEqual(console, "BORROWSER_AG4_READY " + RUN + " ssh-ed25519 TESTPUBLIC\n")
        for state in ["symlink", "dangling-symlink", "file", "owner", "group", "mode", "contents"]:
            with self.subTest(state=state):
                result, console, calls = self.execute(runtime_state=state)
                self.assertEqual(result.returncode, 1, result.stderr.decode())
                self.assertEqual(console.strip(), f"BORROWSER_AG4_BOOTSTRAP_FAILED {RUN} ssh_validate 1")
                self.assertFalse(any(c[0] == "sshd" for c in calls))
                self.assertNotIn(["systemctl", "restart", "ssh"], calls)
                self.assertEqual([c[-1] for c in calls if c[0] == "timeout"], list(PHASES)[:-4])

    @unittest.skipUnless(os.environ.get("BORROWSER_AG4_NOBLE_SSH") == "1",
                         "requires an explicitly disposable root Ubuntu Noble container with OpenSSH")
    def test_disposable_noble_container_generated_ssh_contract(self):
        import pwd
        self.assertEqual(os.geteuid(), 0)
        self.assertTrue(Path("/.dockerenv").exists())
        self.assertIn('VERSION_ID="24.04"', Path("/etc/os-release").read_text())
        self.assertEqual(Path("/proc/1/comm").read_text().strip(), "python3")
        with self.assertRaises(KeyError):
            pwd.getpwnam("ag4")  # Never operate on a pre-existing capture account.
        self.assertFalse(Path("/run/sshd").exists())
        unit = Path("/usr/lib/systemd/system/ssh.service").read_text()
        self.assertIn("RuntimeDirectory=sshd\n", unit)
        self.assertIn("RuntimeDirectoryMode=0755\n", unit)
        routes = json.loads(subprocess.check_output(["ip", "-j", "-4", "route", "show", "default"], timeout=5))
        self.assertEqual(len(routes), 1)
        addresses = json.loads(subprocess.check_output(
            ["ip", "-j", "-4", "address", "show", "dev", routes[0]["dev"], "scope", "global"], timeout=5))
        self.assertEqual(len(addresses), 1)
        self.assertEqual(len(addresses[0]["addr_info"]), 1)
        private = addresses[0]["addr_info"][0]["local"]
        script = json.loads((ROOT / "tests/synthesized.json").read_text())["Resources"]["Runner"]["Properties"]["UserData"]["Fn::Base64"]
        prefix = script[:script.index("\nrun_phase setup ")]
        sequence = "\n".join(line for line in script.splitlines()
                             if line.startswith(("run_phase ssh_", "run_phase ready ")))
        self.assertEqual(re.findall(r"run_phase (\w+) (\d+)", sequence),
                         [(name, str(seconds)) for name, seconds in PHASES.items()
                          if name.startswith("ssh_") or name == "ready"])
        with tempfile.TemporaryDirectory() as directory:
            work = Path(directory); console = work / "console"
            commands = work / "bin"; commands.mkdir()
            restart = commands / "systemctl"
            restart.write_text('#!/bin/sh\n[ "$#" -eq 2 ] && [ "$1" = restart ] && [ "$2" = ssh ] || exit 90\nprintf "%s\\n" "$*" >"$AG4_TEST_RESTART_LOG"\n')
            restart.chmod(0o700)
            restart_log = work / "restart.log"
            controlled = dict(os.environ, PATH=str(commands) + ":" + os.environ["PATH"],
                              AG4_TEST_RESTART_LOG=str(restart_log))
            def run(sequence, env=controlled, real_daemon=False,
                    private_banner=b"SSH-2.0-AG4PrivateTestListener\r\n", private_absent=False):
                console.write_text("")
                restart_log.unlink(missing_ok=True)
                # Only console capture is redirected; generated SSH commands,
                # filesystem paths, policy, phase bounds and validation are exact.
                text = (prefix + "\n" + sequence).replace(">/dev/console", ">" + str(console))
                # The container has no systemd PID 1. The restart helper remains
                # controlled. Both exact port-22 endpoints use actual container
                # addresses; this does not qualify real systemd or the AWS path.
                if real_daemon:
                    return subprocess.run(["/bin/bash", "-c", text], env=env,
                                          capture_output=True, text=True, timeout=60)
                with banner_listener(b"SSH-2.0-AG4TestListener\r\n", port=22):
                    with banner_listener(private_banner, port=22, host=private, absent=private_absent):
                        return subprocess.run(["/bin/bash", "-c", text], env=env,
                                              capture_output=True, text=True, timeout=60)
            def failed(sequence, phase, status, diagnostic, env=controlled):
                result = run(sequence, env)
                self.assertEqual(result.returncode, status, result.stderr)
                self.assertIn(diagnostic, result.stderr)
                self.assertEqual(console.read_text().strip(),
                                 f"BORROWSER_AG4_BOOTSTRAP_FAILED {RUN} {phase} {status}")
                self.assertNotIn("BORROWSER_AG4_READY", console.read_text())
                self.assertFalse(restart_log.exists())
            validation_and_ready = "run_phase ssh_validate 5\nrun_phase ssh_restart 10\nrun_phase ssh_probe_loopback 5\nrun_phase ssh_probe_private 10\nrun_phase ready 5"
            result = run(sequence)
            self.assertEqual(result.returncode, 0, result.stderr)
            public = " ".join(Path("/etc/ssh/ssh_host_ed25519_key.pub").read_text().split()[:2])
            self.assertEqual(console.read_text(), f"BORROWSER_AG4_READY {RUN} {public}\n")
            self.assertEqual(restart_log.read_text(), "restart ssh\n")
            for name, banner, absent in [("refused", None, True), ("silent", None, False),
                                         ("malformed", b"HTTP/1.1 200 OK\r\n", False),
                                         ("oversized", b"SSH-2.0-" + b"x" * 255 + b"\r\n", False)]:
                with self.subTest(private_listener=name):
                    result = run(validation_and_ready, private_banner=banner, private_absent=absent)
                    self.assertEqual(result.returncode, 1, result.stderr)
                    self.assertEqual(console.read_text().strip(),
                                     f"BORROWSER_AG4_BOOTSTRAP_FAILED {RUN} ssh_probe_private 1")
                    self.assertEqual(restart_log.read_text(), "restart ssh\n")
            # Real packaged sshd serves both actual addresses. Wait for its own
            # listening diagnostic, not a sleep/race, before the exact probes.
            daemon = subprocess.Popen(["/usr/sbin/sshd", "-D", "-e"],
                                      stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
            try:
                diagnostic = bytearray()
                deadline = time.monotonic() + 10
                with selectors.DefaultSelector() as selector:
                    selector.register(daemon.stderr, selectors.EVENT_READ)
                    while b"Server listening on 0.0.0.0 port 22." not in diagnostic:
                        remaining = deadline - time.monotonic()
                        self.assertGreater(remaining, 0, diagnostic.decode(errors="replace"))
                        self.assertTrue(selector.select(remaining), diagnostic.decode(errors="replace"))
                        data = os.read(daemon.stderr.fileno(), 1024)
                        self.assertTrue(data, diagnostic.decode(errors="replace"))
                        diagnostic.extend(data)
                        self.assertLessEqual(len(diagnostic), 4096)
                listening = subprocess.check_output(["ss", "-H", "-ltnp", "sport = :22"], timeout=5).decode()
                self.assertIn("0.0.0.0:22", listening)
                self.assertIn(f"pid={daemon.pid},", listening)
                result = run(validation_and_ready, real_daemon=True)
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(console.read_text(), f"BORROWSER_AG4_READY {RUN} {public}\n")
                print("Noble real sshd listener:", listening.strip(), flush=True)
                print("Noble actual private interface:", routes[0]["dev"], private, flush=True)
            finally:
                daemon.terminate()
                try:
                    daemon.wait(timeout=3)
                except subprocess.TimeoutExpired:
                    daemon.kill(); daemon.wait(timeout=3)
                daemon.stderr.close()
            runtime = Path("/run/sshd")
            self.assertFalse(runtime.is_symlink())
            self.assertTrue(runtime.is_dir())
            self.assertEqual((runtime.stat().st_uid, runtime.stat().st_gid), (0, 0))
            self.assertEqual(runtime.stat().st_mode & 0o7777, 0o755)
            self.assertEqual(list(runtime.iterdir()), [])
            user = pwd.getpwnam("ag4")
            key = Path("/home/ag4/.ssh/authorized_keys")
            expected_key = re.search(r"printf '%s' '([^']+)' \| base64 --decode >/home/ag4/.ssh/authorized_keys", script)[1]
            self.assertEqual(key.read_bytes(), base64.b64decode(expected_key))
            for path, mode in [(key, 0o600), (key.parent, 0o700),
                               (Path("/home/ag4/run"), 0o700), (Path("/tmp/ag4"), 0o700)]:
                self.assertEqual(path.stat().st_mode & 0o777, mode)
                self.assertEqual((path.stat().st_uid, path.stat().st_gid), (user.pw_uid, user.pw_gid))
            policy = Path("/etc/ssh/sshd_config.d/00-borrowser-ag4.conf")
            self.assertEqual(policy.read_text(), SSH_POLICY)
            wrapper = Path("/opt/borrowser-ag4/guest_capture.py")
            self.assertEqual(wrapper.read_bytes(), (ROOT / "guest_capture.py").read_bytes())
            self.assertEqual(wrapper.stat().st_mode & 0o777, 0o644)
            # The production validation phase itself created the prerequisite.
            valid = subprocess.run(["sshd", "-t"], capture_output=True, text=True, timeout=10)
            self.assertEqual(valid.returncode, 0, valid.stderr)
            effective = subprocess.run(["sshd", "-T"], capture_output=True, text=True, timeout=10)
            self.assertEqual(effective.returncode, 0, effective.stderr)
            for line in SSH_POLICY.splitlines():
                self.assertIn(line.lower(), effective.stdout.splitlines())
            for state in ["symlink", "file", "owner", "group", "mode", "contents"]:
                with self.subTest(runtime_state=state):
                    if state == "symlink":
                        runtime.rmdir()
                        target = work / "runtime-target"; target.mkdir(mode=0o755)
                        runtime.symlink_to(target)
                    elif state == "file":
                        runtime.rmdir(); runtime.write_bytes(b"must not replace")
                    elif state == "owner": os.chown(runtime, 65534, 0)
                    elif state == "group": os.chown(runtime, 0, 65534)
                    elif state == "mode": runtime.chmod(0o700)
                    else: (runtime / ".unexpected").write_bytes(b"must not erase")
                    previous = runtime.lstat()
                    failed(validation_and_ready, "ssh_validate", 1, "")
                    current = runtime.lstat()
                    self.assertEqual((current.st_ino, current.st_mode, current.st_uid, current.st_gid),
                                     (previous.st_ino, previous.st_mode, previous.st_uid, previous.st_gid))
                    if state == "symlink": self.assertEqual(runtime.readlink(), target)
                    if state == "file": self.assertEqual(runtime.read_bytes(), b"must not replace")
                    if state == "contents": self.assertEqual((runtime / ".unexpected").read_bytes(), b"must not erase")
                    # Restore only this disposable test fixture, never production state.
                    if state in {"symlink", "file"}:
                        runtime.unlink(); runtime.mkdir(mode=0o755)
                    else:
                        if state == "contents": (runtime / ".unexpected").unlink()
                        os.chown(runtime, 0, 0); runtime.chmod(0o755)
            policy.write_text(SSH_POLICY + "Ag4InvalidOption yes\n")
            failed(validation_and_ready, "ssh_validate", 255, "Bad configuration option")
            policy.write_text(SSH_POLICY)
            saved_keys = work / "hostkeys"; saved_keys.mkdir()
            for path in Path("/etc/ssh").glob("ssh_host_*_key"):
                path.rename(saved_keys / path.name)
            self.assertTrue(list(saved_keys.iterdir()))
            failed(validation_and_ready, "ssh_validate", 1, "no hostkeys available")
            for path in saved_keys.iterdir():
                path.rename(Path("/etc/ssh") / path.name)
            # A container without systemd PID 1 cannot qualify a real restart.
            failed("run_phase ssh_restart 10\nrun_phase ready 5", "ssh_restart", 1,
                   "System has not been booted with systemd", env=None)
            result = run(validation_and_ready)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(console.read_text(), f"BORROWSER_AG4_READY {RUN} {public}\n")
            self.assertEqual(restart_log.read_text(), "restart ssh\n")


if __name__ == "__main__":
    unittest.main()
