"""Private foreground process/container ownership for AG4 artifact preparation."""
import ctypes
import errno
import json
import os
from pathlib import Path
import re
import signal
import subprocess
import sys
import tempfile
import time
import uuid


def _status(pid):
    if hasattr(os, "waitid"):
        return os.waitid(os.P_PID, pid, os.WEXITED | os.WNOHANG | os.WNOWAIT)
    # Public Darwin ABI: Python 3.12 may omit os.waitid on macOS.
    class Status(ctypes.Structure):
        _fields_ = [("si_signo", ctypes.c_int), ("si_errno", ctypes.c_int),
                    ("si_code", ctypes.c_int), ("si_pid", ctypes.c_int),
                    ("si_uid", ctypes.c_uint), ("si_status", ctypes.c_int),
                    ("address", ctypes.c_void_p), ("padding", ctypes.c_size_t * 9)]
    status = Status()
    lib = ctypes.CDLL(None, use_errno=True)
    if lib.waitid(1, pid, ctypes.byref(status), 4 | 1 | 32):
        raise OSError(ctypes.get_errno(), "preparation waitid")
    return status if status.si_pid else None


def _running(group, deadline):
    if sys.platform == "darwin":
        # Public libproc PROC_PGRP_ONLY / PROC_PIDTBSDINFO; same supported native
        # observation as the Rust controller, without launching another child.
        lib = ctypes.CDLL("/usr/lib/libproc.dylib", use_errno=True)
        pids = (ctypes.c_int * 65)()
        size = lib.proc_listpids(2, group, pids, ctypes.sizeof(pids))
        if size <= 0 or size >= ctypes.sizeof(pids) or size % 4:
            raise RuntimeError("incomplete preparation group observation")
        class BsdInfo(ctypes.Structure):
            _fields_ = [("prefix", ctypes.c_uint32 * 12), ("comm", ctypes.c_char * 16),
                        ("name", ctypes.c_char * 32), ("nfiles", ctypes.c_uint32),
                        ("pgid", ctypes.c_uint32), ("rest", ctypes.c_uint32 * 4),
                        ("start", ctypes.c_uint64 * 2)]
        active = False
        for pid in pids[:size // 4]:
            if not pid:
                continue
            if time.monotonic() >= deadline:
                raise RuntimeError("preparation group observation deadline")
            info = BsdInfo()
            rc = lib.proc_pidinfo(pid, 3, ctypes.c_uint64(0), ctypes.byref(info), ctypes.sizeof(info))
            if rc == 0 and ctypes.get_errno() == errno.ESRCH:
                continue
            if rc != ctypes.sizeof(info) or info.pgid != group:
                raise RuntimeError("incomplete/conflicting preparation process observation")
            active |= info.prefix[1] != 5  # SZOMB
        return active
    active = False
    members = 0
    with os.scandir("/proc") as entries:
        for index, entry in enumerate(entries):
            if index >= 65536 or time.monotonic() >= deadline:
                raise RuntimeError("preparation process observation bound exceeded")
            if not entry.name.isdecimal():
                continue
            try:
                with open(entry.path + "/stat") as file:
                    text = file.read(8193)
            except FileNotFoundError:
                continue
            fields = text.rsplit(")", 1)[-1].split()
            if len(text) > 8192 or len(fields) < 4:
                raise RuntimeError("incomplete preparation process stat")
            if int(fields[2]) == group:
                members += 1
                if members > 64:
                    raise RuntimeError("preparation group exceeds 64 members")
                active |= fields[0] not in {"Z", "X"}
    return active


class Preparation:
    def __init__(self, source, out):
        if sys.platform not in {"darwin", "linux"} or (sys.platform == "linux" and not hasattr(os, "WNOWAIT")):
            raise RuntimeError("AG4 preparation requires macOS or Linux waitid support")
        self.source, self.out = source, out
        self.deadline = time.monotonic() + 1800
        self.cancelled = None
        self.cleaning = False
        if sys.platform == "linux":
            if ctypes.CDLL(None, use_errno=True).prctl(36, 1, 0, 0, 0):  # PR_SET_CHILD_SUBREAPER
                raise RuntimeError("cannot own preparation descendants")
        for sig in (signal.SIGINT, signal.SIGTERM):
            signal.signal(sig, self._interrupt)

    def _interrupt(self, signum, _frame):
        # Recording instead of throwing also closes the Popen assignment window.
        self.cancelled = signum

    def check_cancelled(self):
        if self.cancelled and not self.cleaning:
            raise InterruptedError("preparation interrupted by signal " + str(self.cancelled))

    def _dispose(self, child):
        deadline = time.monotonic() + 5
        failure = None
        try:
            active = _running(child.pid, deadline)
        except BaseException as error:
            failure, active = error, True
        if active:
            try:
                os.killpg(child.pid, signal.SIGTERM)
            except ProcessLookupError:
                pass
            time.sleep(1)
            try:
                os.killpg(child.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
        # Root identity remains reserved until every signal has been sent.
        while _running(child.pid, deadline):
            if time.monotonic() >= deadline:
                raise RuntimeError("preparation command cleanup unresolved; group " + str(child.pid))
            time.sleep(0.01)
        child.wait(timeout=max(0.001, deadline-time.monotonic()))
        if sys.platform == "linux":
            while True:
                try:
                    pid, _ = os.waitpid(-child.pid, os.WNOHANG)
                except ChildProcessError:
                    break
                if not pid:
                    if time.monotonic() >= deadline:
                        raise RuntimeError("preparation adopted child unresolved")
                    time.sleep(0.01)
        if failure:
            raise failure
        return active

    def run(self, args, *, cwd=None, env=None, data=None, cleanup_deadline=None):
        self.check_cancelled()
        deadline = self.deadline if cleanup_deadline is None else cleanup_deadline
        if time.monotonic() >= deadline:
            raise RuntimeError("artifact preparation deadline exceeded")
        with tempfile.TemporaryFile() as stdin, tempfile.TemporaryFile() as stdout, tempfile.TemporaryFile() as stderr:
            if data is not None:
                stdin.write(data); stdin.seek(0)
            child = None
            primary = None
            try:
                child = subprocess.Popen(args, cwd=cwd or self.source, env=env, stdin=stdin,
                                         stdout=stdout, stderr=stderr, start_new_session=True)
                while True:
                    self.check_cancelled()
                    if time.monotonic() >= deadline or os.fstat(stdout.fileno()).st_size > 4*1024*1024 or os.fstat(stderr.fileno()).st_size > 8*1024*1024:
                        raise RuntimeError("preparation command exceeded its time/output limit: " + args[0])
                    status = _status(child.pid)
                    if status:
                        if status.si_code != 1 or status.si_status:
                            stderr.seek(0); beginning = stderr.read(2048)
                            stderr.seek(max(len(beginning), os.fstat(stderr.fileno()).st_size - 2048))
                            raise RuntimeError(args[0] + " failed: " + (beginning + b"\n...\n" + stderr.read(2048)).decode(errors="replace"))
                        break
                    time.sleep(0.01)
            except BaseException as error:
                primary = error
                raise
            finally:
                if child is not None:
                    try:
                        if self._dispose(child) and primary is None:
                            raise RuntimeError("preparation command left running descendants")
                    except BaseException as error:
                        raise RuntimeError(str(primary or "preparation failed") + "; command cleanup: " + str(error)) from error
            self.check_cancelled()
            stdout.seek(0)
            result = stdout.read(4*1024*1024+1)
            if len(result) > 4*1024*1024:
                raise RuntimeError("preparation output limit exceeded")
            return result

    def container(self, options, command):
        # Identity exists before Docker creation. Only this exact name/label/ID
        # may be removed; no scan for unrelated containers or source packaging.
        token = uuid.uuid4().hex
        name = "borrowser-ag4-prepare-" + token
        record = self.out / "preparation-container.json"
        identity = {"name": name, "label": token, "id": None}
        record.write_text(json.dumps(identity) + "\n")
        primary = None
        try:
            cid = self.run(["docker", "create", "--name", name, "--label", "borrowser.ag4-preparation=" + token,
                            *options, *command]).decode().strip()
            if not re.fullmatch(r"[a-f0-9]{64}", cid):
                raise RuntimeError("invalid preparation container identity")
            identity["id"] = cid
            record.write_text(json.dumps(identity) + "\n")
            return self.run(["docker", "start", "--attach", cid])
        except BaseException as error:
            primary = error
            raise
        finally:
            self.cleaning = True
            limit = time.monotonic() + 30
            try:
                def docker(*args):
                    return self.run(["docker", *args], cleanup_deadline=limit)
                def lookup():
                    return docker("container", "ls", "--all", "--no-trunc", "--filter", "name=^/" + name + "$", "--format", "{{.ID}}").decode().splitlines()
                ids = lookup()
                if not ids and identity["id"] is None:
                    raise RuntimeError("container creation outcome unresolved")
                if ids:
                    if len(ids) != 1 or not re.fullmatch(r"[a-f0-9]{64}", ids[0]):
                        raise RuntimeError("ambiguous preparation container observation")
                    observed = json.loads(docker("inspect", ids[0], "--format", "{{json .}}"))
                    if (observed.get("Id") != ids[0] or observed.get("Name") != "/" + name
                        or observed.get("Config", {}).get("Labels", {}).get("borrowser.ag4-preparation") != token
                        or identity["id"] not in (None, ids[0])):
                        raise RuntimeError("preparation container ownership conflict")
                    docker("rm", "--force", ids[0])
                    if lookup():
                        raise RuntimeError("preparation container removal unresolved")
                record.unlink()
            except BaseException as error:
                raise RuntimeError(str(primary or "container completed") + "; container cleanup: " + str(error)
                                   + "; recovery identity retained at " + str(record)) from error
            finally:
                self.cleaning = False
