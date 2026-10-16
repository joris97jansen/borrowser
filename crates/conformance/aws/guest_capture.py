#!/usr/bin/python3
"""One AG4 guest capture. Stream limits never become browser file limits."""
import argparse
import ctypes
import os
from pathlib import Path
import selectors
import signal
import subprocess
import sys
import time

REPORT_LIMIT = 65536
DIAGNOSTIC_LIMIT = 262144


def _status(pid):
    if hasattr(os, "waitid"):
        return os.waitid(os.P_PID, pid, os.WEXITED | os.WNOHANG | os.WNOWAIT)
    # Python 3.12 on macOS does not expose waitid. Native wrapper qualification
    # still retains the root using the public Darwin siginfo_t / waitid ABI.
    if sys.platform != "darwin":
        raise RuntimeError("unsupported guest-wrapper qualification platform")
    class Status(ctypes.Structure):
        _fields_ = [("si_signo", ctypes.c_int), ("si_errno", ctypes.c_int),
                    ("si_code", ctypes.c_int), ("si_pid", ctypes.c_int),
                    ("si_uid", ctypes.c_uint), ("si_status", ctypes.c_int),
                    ("address", ctypes.c_void_p), ("padding", ctypes.c_size_t * 9)]
    status = Status()
    lib = ctypes.CDLL(None, use_errno=True)
    if lib.waitid(1, pid, ctypes.byref(status), 4 | 1 | 32):
        raise OSError(ctypes.get_errno(), "guest waitid")
    return status if status.si_pid else None


class Cancellation:
    def __init__(self):
        self.first = None
        self.count = 0

    def record(self, signum, _frame):
        # Never throw through Popen acquisition, signaling, waiting or publication.
        if self.first is None:
            self.first = signum
        self.count += 1

    def check(self):
        if self.first is not None:
            raise RuntimeError("capture cancelled")


def capture(harness, chromium, fixture, cancellation):
    # The private working directory and binaries are prepared by bootstrap/SSH.
    # These paths are explicit so qualification runs this exact wrapper unchanged.
    if Path("capture.json").exists():
        raise RuntimeError("capture already published")
    with open("capture.started", "xb"):
        pass
    command = [harness, "--chromium", "--chromium-executable", chromium]
    if fixture:
        command.append(fixture)
    cancellation.check()
    child = None
    succeeded = False
    primary = None
    cleanup_error = None
    with open("capture.part", "xb") as report, open("capture.stderr", "xb") as diagnostic, selectors.DefaultSelector() as streams:
        try:
            child = subprocess.Popen(command, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                                     stderr=subprocess.PIPE, start_new_session=True)
            cancellation.check()
            for pipe, target, limit in [(child.stdout, report, REPORT_LIMIT),
                                        (child.stderr, diagnostic, DIAGNOSTIC_LIMIT)]:
                os.set_blocking(pipe.fileno(), False)
                streams.register(pipe, selectors.EVENT_READ, [target, limit, 0])
            deadline = time.monotonic() + 110
            while streams.get_map():
                cancellation.check()
                if time.monotonic() >= deadline:
                    raise RuntimeError("capture stream deadline exceeded")
                for key, _ in streams.select(min(0.05, max(0, deadline - time.monotonic()))):
                    data = os.read(key.fd, 8192)
                    if not data:
                        streams.unregister(key.fileobj)
                        continue
                    target, limit, count = key.data
                    target.write(data[:max(0, limit-count)])
                    key.data[2] += len(data)
                    if key.data[2] > limit:
                        raise RuntimeError("capture " + ("report" if target is report else "diagnostic") + " overflow")
            # Keep the root unreaped until all required group signaling is done.
            while not _status(child.pid):
                cancellation.check()
                if time.monotonic() >= deadline:
                    raise RuntimeError("capture execution deadline exceeded")
                time.sleep(0.01)
            status = _status(child.pid)
            if status.si_code != 1 or status.si_status != 0:  # CLD_EXITED
                raise RuntimeError("capture harness failed: wait status " + str(status.si_status))
            cancellation.check()
            succeeded = True
        except BaseException as error:
            primary = error
        finally:
            if child is not None:
                # Ordinary cancellation reaches the AG2 harness first, giving its
                # existing native Chromium owner time to clean up. A forced guest
                # failure is never cleanup proof; the controller deletes the stack.
                if not succeeded or cancellation.first is not None:
                    for sig, delay in [(signal.SIGTERM, 1), (signal.SIGKILL, 0)]:
                        try:
                            os.killpg(child.pid, sig)
                        except ProcessLookupError:
                            pass
                        except OSError as error:
                            cleanup_error = cleanup_error or error
                        if delay:
                            time.sleep(delay)
                try:
                    child.wait(timeout=4)
                except BaseException as error:
                    cleanup_error = cleanup_error or error
                child.stdout.close()
                child.stderr.close()
    published = False
    try:
        if primary is None and cleanup_error is None:
            cancellation.check()
            os.replace("capture.part", "capture.json")
            published = True
            # A signal delivered during rename records state without losing our
            # publication ownership. Remove this invocation's result on failure.
            cancellation.check()
    except BaseException as error:
        primary = error
    if primary or cleanup_error or cancellation.first is not None:
        if published:
            try:
                Path("capture.json").unlink()
            except OSError as error:
                cleanup_error = cleanup_error or error
        details = ["primary=" + str(primary or "none")]
        if cancellation.first is not None:
            details.append("interruption=signal " + str(cancellation.first) + " (" + str(cancellation.count) + " handled)")
        if cleanup_error:
            details.append("guest command cleanup=" + str(cleanup_error))
        raise RuntimeError("; ".join(details))


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--harness", required=True)
    parser.add_argument("--chromium", required=True)
    parser.add_argument("fixture", nargs="?")
    args = parser.parse_args()
    cancellation = Cancellation()
    signal.signal(signal.SIGINT, cancellation.record)
    signal.signal(signal.SIGTERM, cancellation.record)
    os.umask(0o077)
    try:
        capture(args.harness, args.chromium, args.fixture, cancellation)
    except BaseException as error:
        print("AG4 guest capture failed: " + str(error)[:1024], file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
