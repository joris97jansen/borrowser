import json
import os
import select
import signal
import time
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

WRAPPER = Path(__file__).resolve().parents[1] / "guest_capture.py"


class GuestCapture(unittest.TestCase):
    def test_stream_limits_do_not_limit_browser_state_and_failures_never_publish(self):
        for mode in ("success", "report", "diagnostic", "partial", "late-overflow"):
            with self.subTest(mode=mode), tempfile.TemporaryDirectory() as directory:
                work = Path(directory)
                helper = work / "harness"
                helper.write_text("#!" + sys.executable + "\n" + '''
import pathlib, sys
pathlib.Path("browser-state").write_bytes(b"x" * 1024*1024)
mode = ''' + repr(mode) + '''
if mode == "report": sys.stdout.write("x" * 65537)
elif mode == "diagnostic": sys.stderr.write("x" * 262145)
elif mode == "partial": sys.stdout.write('{"partial":'); sys.exit(3)
elif mode == "late-overflow": sys.stdout.write("x" * 1000000)
else: print('{"complete":true}')
''')
                helper.chmod(0o700)
                result = subprocess.run([sys.executable, str(WRAPPER), "--harness", str(helper),
                                         "--chromium", "/unused/pinned/chromium"], cwd=work, capture_output=True, timeout=10)
                self.assertEqual((work / "browser-state").stat().st_size, 1024*1024)
                self.assertLessEqual((work / "capture.part").stat().st_size if (work / "capture.part").exists() else 0, 65536)
                self.assertLessEqual((work / "capture.stderr").stat().st_size, 262144)
                self.assertEqual(result.returncode == 0, mode == "success", result.stderr.decode())
                self.assertEqual((work / "capture.json").exists(), mode == "success")
                if mode == "success":
                    self.assertEqual(json.loads((work / "capture.json").read_bytes()), {"complete": True})
                    again = subprocess.run([sys.executable, str(WRAPPER), "--harness", str(helper), "--chromium", "/unused"], cwd=work, capture_output=True, timeout=10)
                    self.assertNotEqual(again.returncode, 0)
                elif mode != "partial":
                    self.assertIn(b"overflow", result.stderr)

    def test_cancellation_preserves_child_ownership_cleanup_and_primary_failure(self):
        for mode in ("ordinary", "overflow-grace", "failure-grace", "acquisition",
                     "before-publication", "during-publication", "repeated", "cleanup-error"):
            with self.subTest(mode=mode), tempfile.TemporaryDirectory() as directory:
                self.cancellation_case(Path(directory), mode)

    def cancellation_case(self, work, mode):
        # Test-only interposition observes real ownership transitions. Pipe
        # handshakes let the parent deliver signals at those exact transitions;
        # no production switches or sleeps guessing at a race window.
        instrument = work / "instrument"
        instrument.mkdir()
        (instrument / "sitecustomize.py").write_text(INSTRUMENTATION)
        helper = work / "harness"
        helper.write_text("#!" + sys.executable + "\n" + HARNESS)
        helper.chmod(0o700)
        notify_read, notify_write = os.pipe()
        release_read, release_write = os.pipe()
        environment = dict(os.environ, PYTHONPATH=str(instrument),
                           AG4_TEST_MODE=mode, AG4_TEST_NOTIFY=str(notify_write),
                           AG4_TEST_RELEASE=str(release_read), PYTHONDONTWRITEBYTECODE="1")
        child = subprocess.Popen([sys.executable, str(WRAPPER), "--harness", str(helper),
                                  "--chromium", "/unused/pinned/chromium"], cwd=work,
                                 env=environment, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                 pass_fds=(notify_write, release_read), start_new_session=True)
        os.close(notify_write)
        os.close(release_read)
        harness_pid = None
        try:
            for index in range(3 if mode == "repeated" else 1):
                self.assertTrue(select.select([notify_read], [], [], 8)[0], "transition not reached")
                marker = os.read(notify_read, 1)
                self.assertEqual(marker, b"R", "wrapper exited before transition")
                harness_pid = int((work / "harness-pid").read_text())
                child.send_signal(signal.SIGTERM if index != 1 else signal.SIGINT)
                os.write(release_write, b"C")
            output, error = child.communicate(timeout=8)
            self.assertNotEqual(child.returncode, 0, (output, error))
            self.assertIn(b"interruption=signal", error)
            self.assertFalse((work / "capture.json").exists())
            self.assertLessEqual((work / "capture.part").stat().st_size if (work / "capture.part").exists() else 0, 65536)
            self.assertLessEqual((work / "capture.stderr").stat().st_size, 262144)
            if mode in ("overflow-grace", "repeated"):
                self.assertIn(b"primary=capture report overflow", error)
            elif mode == "failure-grace":
                self.assertIn(b"primary=controlled execution failure", error)
            elif mode == "cleanup-error":
                self.assertIn(b"primary=capture report overflow", error)
                self.assertIn(b"guest command cleanup=", error)
            if mode == "repeated":
                self.assertIn(b"(3 handled)", error)
            with self.assertRaises(ProcessLookupError):
                os.kill(harness_pid, 0)  # wrapper waited/reaped before reporting failure
        finally:
            if child.poll() is None:
                os.killpg(child.pid, signal.SIGKILL)
                child.communicate(timeout=5)
            if harness_pid is None and (work / "harness-pid").exists():
                harness_pid = int((work / "harness-pid").read_text())
            if harness_pid is not None:
                try:
                    os.killpg(harness_pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
            os.close(notify_read)
            os.close(release_write)

    @unittest.skipUnless(os.environ.get("BORROWSER_AG4_GUEST_HARNESS") and os.environ.get("BORROWSER_CHROMIUM_EXECUTABLE"), "opt-in exact wrapper / pinned Chromium qualification")
    def test_real_pinned_chromium_through_exact_guest_wrapper(self):
        pin = json.loads((WRAPPER.parent.parent / "chromium-reference.json").read_text())
        with tempfile.TemporaryDirectory() as directory:
            result = subprocess.run([sys.executable, str(WRAPPER), "--harness", os.environ["BORROWSER_AG4_GUEST_HARNESS"],
                                     "--chromium", os.environ["BORROWSER_CHROMIUM_EXECUTABLE"]], cwd=directory, capture_output=True, timeout=125)
            diagnostic = (Path(directory) / "capture.stderr").read_bytes()
            self.assertEqual(result.returncode, 0, (result.stderr + diagnostic).decode(errors="replace"))
            raw = (Path(directory) / "capture.json").read_bytes()
            self.assertLessEqual(len(raw), 65536)
            report = json.loads(raw)
            self.assertEqual(report["schema"], "borrowser.chromium-phases.v2")
            self.assertTrue(report["observations"])
            observed_ids = [(o["descriptor"]["case_id"], o["descriptor"]["role"] == "reference") for o in report["observations"]]
            self.assertEqual(observed_ids, sorted(set(observed_ids)))
            ids = ["canvas/root", "canvas/cascade", "dom/structure", "dom/template-namespaces",
                   "style/cascade-inheritance", "layout/block-boxes"]
            for index, fixture_id in enumerate(ids):
                observation = next(o for o in report["observations"] if o["descriptor"]["case_id"] == fixture_id and o["descriptor"]["role"] == "test")
                if index < 2:
                    self.assertIsNone(observation["inspection"])
                    self.assertEqual(observation["canvas_color_rgb8"], [[18, 52, 86], [52, 86, 120]][index])
                else:
                    self.assertIsNone(observation["canvas_color_rgb8"])
                    inspection = observation["inspection"]
                    self.assertEqual(inspection["inner_width"], 640)
                    self.assertEqual(inspection["inner_height"], 480)
                    self.assertGreater(inspection["layout_width"], 0)
                    self.assertLessEqual(inspection["layout_width"], inspection["inner_width"])
                    self.assertEqual(inspection["layout_width"], int(inspection["layout_width"]))
                    self.assertEqual(isinstance(inspection["dom"], dict), index < 4)
                    self.assertEqual([s["target"] for s in inspection["styles"]],
                                     ["parent", "child", "hidden"] if index == 4 else [])
                    self.assertTrue(all(len(s["values"]) == 4 for s in inspection["styles"]))
                    self.assertEqual([l["target"] for l in inspection["layout"]],
                                     ["container", "child", "sibling", "hidden", "zero", "missing"] if index == 5 else [])
                self.assertEqual(observation["browser"]["version"], pin["version"])
                self.assertEqual(observation["browser"]["revision"], pin["revision"])
                self.assertEqual(observation["browser"]["capture_profile"], pin["capture_profile"])
            for fixture_id, target in [("wpt/rgb-comments-p6", "p6"), ("wpt/rgb-alias-p6", "p6"), ("wpt-style/background-color-mismatch", "probe")]:
                rows = [o for o in report["observations"] if o["descriptor"]["case_id"] == fixture_id]
                self.assertEqual([o["descriptor"]["role"] for o in rows], ["test", "reference"])
                for row in rows:
                    self.assertEqual(len(row["descriptor"]["source_sha256"]), 64)
                    self.assertEqual(row["inspection"]["styles"][0]["target"], target)
                    expected_color = "rgb(10, 150, 50)" if target == "p6" else ("rgb(0, 128, 0)" if row["descriptor"]["role"] == "test" else "rgb(255, 0, 0)")
                    self.assertEqual(row["inspection"]["styles"][0]["values"][1], expected_color)
            self.assertFalse(any(o["descriptor"]["case_id"].startswith("wpt/unsupported/") for o in report["observations"]))
            # This records the observed platform; macOS evidence is not Linux/AWS evidence.
            print("AG4 exact guest wrapper platform:", report["observations"][0]["browser"]["platform"])


HARNESS = r"""
import os, pathlib, signal, sys, time
signal.signal(signal.SIGTERM, signal.SIG_IGN)
signal.signal(signal.SIGINT, signal.SIG_IGN)
pathlib.Path("harness-pid").write_text(str(os.getpid()))
pathlib.Path("harness-ready").touch()
mode = os.environ["AG4_TEST_MODE"]
if mode in ("before-publication", "during-publication"):
    print('{"complete":true}')
    sys.exit(0)
if mode in ("overflow-grace", "repeated", "cleanup-error"):
    sys.stdout.write("x" * 65537)
    sys.stdout.flush()
while True:
    time.sleep(1)
"""

INSTRUMENTATION = r"""
import os, pathlib, selectors, signal, subprocess, sys, time
if pathlib.Path(sys.argv[0]).name == "guest_capture.py":
    mode = os.environ["AG4_TEST_MODE"]
    def handshake():
        os.write(int(os.environ["AG4_TEST_NOTIFY"]), b"R")
        if os.read(int(os.environ["AG4_TEST_RELEASE"]), 1) != b"C":
            raise RuntimeError("test handshake abandoned")
        # Wait until the Python handler has actually run, not just until a signal
        # was sent. This confines synchronization to the test interpreter.
        handler = signal.getsignal(signal.SIGTERM)
        deadline = time.monotonic() + 3
        while handler.__self__.count < handshake.count + 1:
            if time.monotonic() >= deadline:
                raise RuntimeError("test signal delivery deadline")
            time.sleep(0.001)
        handshake.count += 1
    handshake.count = 0
    original_popen = subprocess.Popen
    def acquire(*args, **kwargs):
        child = original_popen(*args, **kwargs)
        deadline = time.monotonic() + 3
        while not pathlib.Path("harness-ready").exists():
            if time.monotonic() >= deadline:
                raise RuntimeError("test harness readiness deadline")
            time.sleep(0.001)
        if mode == "acquisition":
            handshake()  # real Popen returned, wrapper has not acquired its handle
        return child
    subprocess.Popen = acquire
    original_select = selectors.DefaultSelector.select
    def select(self, *args, **kwargs):
        if mode == "ordinary" and handshake.count == 0:
            handshake()
        if mode == "failure-grace":
            raise RuntimeError("controlled execution failure")
        return original_select(self, *args, **kwargs)
    selectors.DefaultSelector.select = select
    original_sleep = time.sleep
    def sleep(delay):
        if delay == 1 and mode in ("overflow-grace", "failure-grace", "repeated", "cleanup-error"):
            handshake()  # in actual TERM grace; root deliberately ignores TERM
            if mode == "repeated":
                handshake()
                handshake()
        original_sleep(delay)
    time.sleep = sleep
    original_killpg = os.killpg
    def killpg(pid, sig):
        original_killpg(pid, sig)
        if sig == signal.SIGTERM and mode == "cleanup-error":
            raise OSError("controlled TERM observation error")
    os.killpg = killpg
    original_replace = os.replace
    def replace(source, destination):
        if mode == "before-publication":
            handshake()
        original_replace(source, destination)
        if mode == "during-publication":
            handshake()
    os.replace = replace
"""


if __name__ == "__main__":
    unittest.main()
