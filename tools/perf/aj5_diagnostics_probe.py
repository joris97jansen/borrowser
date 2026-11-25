#!/usr/bin/env python3
"""AJ5-only verified-input, bounded, five-pair qualification. Never retries/builds."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import selectors
import signal
import socket
import subprocess
import time

PROCESS_SECONDS = 1800
CASE_SECONDS = 600
CAMPAIGN_SECONDS = 10800
CLEANUP_SECONDS = 10
PIPE_BYTES = 65536
CASES = {(n, k, w) for n in (65536, 1048576) for k in ("clean", "late", "heavy") for w in (1, 64, 1024)}


def input_bytes(size, kind):
    """Materialize the shared external corpus once; the Rust probe never generates it."""
    source = b"<!doctype html><body>"
    if kind == "heavy":
        group = b"<div>" + b"<span a='first' a='second'></span>" * 32 + b"</div>"
        source += group * ((size - len(source)) // len(group))
    tail = b"<p a=x a=y>" if kind == "late" else b""
    return source + b"\n" * (size - len(source) - len(tail)) + tail


def input_name(size, kind):
    return f"{size}-{kind}.html"


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def load_inputs(directory, manifest=None):
    identities = {}
    for n, k in sorted({case[:2] for case in CASES}):
        path = directory / input_name(n, k)
        if path.is_symlink() or not path.is_file() or path.stat().st_size != n:
            raise ValueError("invalid external corpus entry")
        identities[(n, k)] = digest(path)
    if manifest is not None:
        if manifest.is_symlink() or manifest.stat().st_size > 1024:
            raise ValueError("invalid shared corpus manifest")
        rows=[line.split() for line in manifest.read_text(encoding="ascii").splitlines()]
        expected={input_name(n,k):value for (n,k),value in identities.items()}
        if len(rows)!=6 or any(len(row)!=2 for row in rows) or {row[1]:row[0] for row in rows}!=expected:
            raise ValueError("shared corpus manifest mismatch")
    return identities


class ProbeInterrupted(BaseException):
    pass


def leader_exited(process):
    # Do not reap: the owned child pins its PID/PGID until our one group signal.
    info = os.waitid(os.P_PID, process.pid, os.WEXITED | os.WNOHANG | os.WNOWAIT)
    return info is not None and info.si_pid == process.pid


def group_exists(pgid):
    try:
        os.killpg(pgid, 0)
        return True
    except ProcessLookupError:
        return False


def cleanup_group(process):
    deadline = time.monotonic() + CLEANUP_SECONDS
    result = {"pgid": process.pid, "signal_while_leader_owned": False,
              "leader_reaped": False, "verified_absent": False, "failure": None}
    status = None
    try:
        # start_new_session made our still-unreaped child the session/group
        # leader. Never signal an unverified/reused group after reaping it.
        # Darwin can report ESRCH for a zombie's getpgid/getsid even though
        # waitid(WNOWAIT) still pins our child identity. start_new_session and
        # that unreaped ownership establish the group in this exited case.
        if not leader_exited(process):
            try:
                if os.getpgid(process.pid) != process.pid or os.getsid(process.pid) != process.pid:
                    raise OSError("owned process group identity changed")
            except ProcessLookupError:
                if not leader_exited(process):
                    raise
        try:
            os.killpg(process.pid, signal.SIGKILL)
            result["signal_while_leader_owned"] = True
        except OSError as error:
            # Darwin reports EPERM for a group containing only its unreaped
            # zombie. Record it, reap, then require actual group absence. A live
            # survivor or permission error during verification still fails.
            result["signal_error"] = str(error)
        status = process.wait(timeout=max(0, deadline - time.monotonic()))
        result["leader_reaped"] = True
        while True:
            try:
                if not group_exists(process.pid):
                    break
            except PermissionError as error:
                # A just-killed Darwin zombie can yield EPERM until launchd
                # reaps it. This is uncertainty, never absence: query only,
                # under the original cleanup deadline; never signal again.
                result.setdefault("verification_error", str(error))
            if time.monotonic() >= deadline:
                raise TimeoutError(result.get("verification_error", "owned process group remains after cleanup"))
            time.sleep(min(.01, max(0, deadline - time.monotonic())))
        result["verified_absent"] = True
    except (OSError, subprocess.TimeoutExpired, TimeoutError) as error:
        result["failure"] = str(error) or "cleanup-timeout"
    return status, result


def supervise(argv, *, inputs, process_seconds=PROCESS_SECONDS, case_seconds=CASE_SECONDS,
              output_limit=PIPE_BYTES, progress_path=None, measurement_records=True):
    if not all(hasattr(os, name) for name in ("waitid", "WNOWAIT", "WEXITED", "P_PID")):
        raise RuntimeError("non-reaping child observation is required before spawning")
    started = time.monotonic()
    process = selector = None
    streams = [bytearray(), bytearray()]
    failure = None
    rows, records = [], []
    pending = bytearray()
    active = None
    active_since = started
    status = None
    cleanup = {"verified_absent": True, "failure": None, "no_child": True}
    signals = (signal.SIGTERM, signal.SIGINT)
    previous_handlers = {sig: signal.getsignal(sig) for sig in signals}
    previous_mask = None

    def interrupted(signum, frame):
        raise ProbeInterrupted(f"signal {signum}")

    try:
        for sig in signals:
            signal.signal(sig, interrupted)
        previous_mask = signal.pthread_sigmask(signal.SIG_BLOCK, signals)
        process = subprocess.Popen(argv, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                   start_new_session=True)
        signal.pthread_sigmask(signal.SIG_SETMASK, previous_mask)
        selector = selectors.DefaultSelector()
        for index, pipe in enumerate((process.stdout, process.stderr)):
            os.set_blocking(pipe.fileno(), False)
            selector.register(pipe, selectors.EVENT_READ, index)
        while selector.get_map() or not leader_exited(process):
            now = time.monotonic()
            if now - started >= process_seconds or now - active_since >= case_seconds:
                failure = "timeout"
                break
            for key, _ in selector.select(min(.1, process_seconds - (now - started))):
                data = os.read(key.fd, 8192)
                if not data:
                    selector.unregister(key.fileobj)
                    key.fileobj.close()
                    continue
                stream = streams[key.data]
                if len(stream) + len(data) > output_limit:
                    stream.extend(data[:max(0, output_limit - len(stream))])
                    failure = "output-limit"
                    break
                stream.extend(data)
                if key.data == 0 and measurement_records:
                    pending.extend(data)
                    while b"\n" in pending:
                        line, _, tail = pending.partition(b"\n")
                        pending = bytearray(tail)
                        fields = line.decode("ascii").split()
                        if len(fields) != 6 or fields[0] not in ("AJ5_CASE_START", "AJ5_CASE_END"):
                            raise ValueError("unexpected probe output")
                        case = (int(fields[1]), fields[2], int(fields[3]))
                        if case not in CASES or fields[5] != inputs.get(case[:2]):
                            raise ValueError("case/input SHA-256 mismatch")
                        if fields[0] == "AJ5_CASE_START":
                            if fields[4] != "1:1" or active is not None or case in {r[:3] for r in rows}:
                                raise ValueError("invalid/duplicate case start")
                            active, active_since = case, time.monotonic()
                        else:
                            if active != case or len(fields[4])>39 or not 0<int(fields[4])<=int(case_seconds*1_000_000_000):
                                raise ValueError("case end without matching start")
                            rows.append((*case, int(fields[4])))
                            active = None
                            active_since = time.monotonic()
                        record = {"event": fields[0], "case": case, "value": fields[4], "sha256": fields[5]}
                        records.append(record)
                        if progress_path is not None:
                            # Exactly two fixed-size records per each of 18 cases.
                            with progress_path.open("a", encoding="ascii") as log:
                                log.write(json.dumps(record) + "\n")
            if failure:
                break
        if pending or active is not None:
            failure = failure or "unterminated-case-or-output"
    except (ValueError, UnicodeError, OSError, ChildProcessError, KeyboardInterrupt, ProbeInterrupted) as error:
        failure = str(error) or type(error).__name__
    finally:
        for sig in signals:
            signal.signal(sig, signal.SIG_IGN)
        if process is not None:
            # Includes successful leader exit with redirected descendant pipes.
            status, cleanup = cleanup_group(process)
            if not cleanup["verified_absent"]:
                failure = failure or "cleanup-unverified"
            for pipe in (process.stdout, process.stderr):
                if pipe is not None:
                    pipe.close()
        if selector is not None:
            selector.close()
        if previous_mask is not None:
            signal.pthread_sigmask(signal.SIG_SETMASK, previous_mask)
        for sig in signals:
            signal.signal(sig, previous_handlers[sig])
    if status != 0:
        failure = failure or "exit-status"
    if measurement_records and ({r[:3] for r in rows} != CASES or len(records) != 36):
        failure = failure or "incomplete-case-set"
    return {"argv": argv, "exit": status, "failure": failure, "rows": rows,
            "records": records, "cleanup": cleanup, "seconds": time.monotonic() - started,
            "stdout": bytes(streams[0]).decode("utf-8", "replace"),
            "stderr": bytes(streams[1]).decode("utf-8", "replace")}


def source_fingerprint(directory):
    paths = subprocess.check_output(["git", "ls-files", "--cached", "--others", "--exclude-standard", "-z"], cwd=directory).split(b"\0")
    h = hashlib.sha256()
    for raw in sorted(set(paths)):
        if raw:
            path = directory / os.fsdecode(raw)
            if path.is_file():
                h.update(raw + b"\0" + path.read_bytes() + b"\0")
    return h.hexdigest()


def identities(args, receipts):
    return {"probe_sha256": digest(args.probe_source),
            "executables": {name: digest(getattr(args, name)) for name in ("baseline", "candidate")},
            "sources": {name: source_fingerprint(Path(receipts[name]["cwd"])) for name in ("baseline", "candidate")},
            "inputs": {f"{n}-{k}": value for (n, k), value in load_inputs(args.input_directory,args.input_manifest).items()},
            "input_manifest_sha256": digest(args.input_manifest),
            "build_inputs": {name: {
                "receipt_sha256":digest(getattr(args,name+"_receipt")),
                "manifest_sha256":digest(Path(receipt["manifest_path"])),
                "lock_sha256":digest(Path(receipt["manifest_path"]).parent/"Cargo.lock")
            } for name,receipt in receipts.items()}}


def run_campaign(args):
    if args.warmups != 1 or args.samples != 1:
        raise ValueError("approved campaign requires exactly one warmup and one sample")
    args.evidence.mkdir(exist_ok=False)
    receipts = {name: json.loads(getattr(args, name + "_receipt").read_text()) for name in ("baseline", "candidate")}
    before = identities(args, receipts)
    for name, receipt in receipts.items():
        if (receipt["executable_sha256"] != before["executables"][name]
                or receipt["probe_sha256"] != before["probe_sha256"]
                or receipt["source_fingerprint"] != before["sources"][name]
                or receipt["source_fingerprint"] != getattr(args, name + "_source")
                or receipt["exit"] != 0 or not receipt["source_unchanged"]
                or receipt["manifest_sha256"] != before["build_inputs"][name]["manifest_sha256"]
                or receipt["lock_sha256"] != before["build_inputs"][name]["lock_sha256"]):
            raise ValueError("build receipt identity mismatch")
    metadata = {"before": before, "complete": False, "build_receipts": receipts,
                "warmups": args.warmups, "samples": args.samples, "input_directory": str(args.input_directory),"input_manifest":str(args.input_manifest),
                "limits": {"case_seconds": CASE_SECONDS, "process_seconds": PROCESS_SECONDS,
                           "campaign_seconds": CAMPAIGN_SECONDS, "cleanup_seconds": CLEANUP_SECONDS,
                           "pipe_bytes": PIPE_BYTES}, "accepted_measurements": 0}
    record = args.evidence / "campaign.json"
    record.write_text(json.dumps(metadata, indent=2) + "\n")
    deadline = time.monotonic() + CAMPAIGN_SECONDS
    try:
        for pair in range(5):
            order = ("baseline", "candidate") if pair % 2 == 0 else ("candidate", "baseline")
            for name in order:
                if time.monotonic() >= deadline:
                    raise ValueError("campaign timeout; incomplete")
                if identities(args, receipts) != before:
                    raise ValueError("identity drift before process")
                argv = [str(getattr(args, name)), "--input-directory", str(args.input_directory),
                        "--input-manifest",str(args.input_manifest),"--warmups", str(args.warmups), "--samples", str(args.samples)]
                result = supervise(argv, inputs=load_inputs(args.input_directory,args.input_manifest),
                                   process_seconds=min(PROCESS_SECONDS, deadline - time.monotonic()),
                                   progress_path=args.evidence / f"pair-{pair}-{name}-progress.jsonl")
                result["identities_after"] = identities(args, receipts)
                (args.evidence / f"pair-{pair}-{name}.json").write_text(json.dumps(result, indent=2) + "\n")
                if (result["failure"] or not result["cleanup"]["verified_absent"]
                        or result["exit"] != 0 or len(result["rows"]) != 18 or {tuple(row[:3])for row in result["rows"]} != CASES
                        or len(result["records"]) != 36 or result["identities_after"] != before):
                    raise ValueError("campaign incomplete; measurement/cleanup/identity failure; no retry")
                metadata["accepted_measurements"] += len(result["rows"])
        metadata["after"] = identities(args, receipts)
        if metadata["after"] != before or metadata["accepted_measurements"] != 180:
            raise ValueError("campaign incomplete; final identity/count failure")
        metadata["complete"] = True
    except (ValueError, OSError, ProbeInterrupted, KeyboardInterrupt) as error:
        metadata["failure"] = str(error) or type(error).__name__
        raise
    finally:
        record.write_text(json.dumps(metadata, indent=2) + "\n")
    return metadata


def qualify_smoke(executable, evidence):
    """Only the AJ5 loopback fixture and its four explicit failure cases."""
    evidence.mkdir(exist_ok=False)
    identity=digest(executable)
    for mode in ("", "before-navigation", "after-navigation", "after-first-terminal", "abort"):
        if digest(executable)!=identity:
            raise ValueError("smoke executable drift")
        argv=[str(executable)]+([mode]if mode else[])
        result=supervise(argv,inputs={},process_seconds=70,case_seconds=70,measurement_records=False)
        result["expected_failure_fixture"]=mode or None
        result["executable_sha256"]=identity
        output=result["stdout"]
        server=next((line.split()[1]for line in output.splitlines()if line.startswith("AJ5_SMOKE_SERVER ")),None)
        closed=False
        if server is not None:
            host,port=server.rsplit(":",1)
            try:
                with socket.create_connection((host,int(port)),timeout=.2):pass
            except ConnectionRefusedError:closed=True
        result["listener_verified_closed"]=closed
        result["executable_unchanged"]=digest(executable)==identity
        clean=(result["cleanup"]["verified_absent"] and closed and result["executable_unchanged"]
               and result["failure"] == ("exit-status" if mode else None))
        if mode=="abort":
            accepted=clean and result["exit"] not in (None,0) and "AJ5 controlled optimized-abort fixture"in result["stderr"]
        else:
            accepted=clean and result["exit"]==(1 if mode else 0) and "server=Ok(()) runtime=Ok(()) cleanup_verified=true"in output
            accepted=accepted and ('primary=Err("injected loopback failure")'in output if mode else "primary=Ok(())"in output)
        result["qualification_passed"]=accepted
        (evidence/((mode or "success")+".json")).write_text(json.dumps(result,indent=2)+"\n")
        if not accepted:
            raise ValueError("loopback qualification incomplete; no retry")


def main():
    if "--smoke-binary" in os.sys.argv[1:]:
        parser=argparse.ArgumentParser("AJ5 loopback success/failure qualification")
        parser.add_argument("--smoke-binary",type=Path,required=True)
        parser.add_argument("--smoke-evidence",type=Path,required=True)
        args=parser.parse_args()
        try:qualify_smoke(args.smoke_binary,args.smoke_evidence)
        except (ValueError,OSError,ProbeInterrupted,KeyboardInterrupt)as error:raise SystemExit(str(error))from error
        return
    parser = argparse.ArgumentParser(__doc__)
    for name in ("baseline", "candidate", "probe-source", "baseline-receipt", "candidate-receipt", "evidence", "input-directory", "input-manifest"):
        parser.add_argument("--" + name, type=Path, required=True)
    for name in ("baseline-source", "candidate-source"):
        parser.add_argument("--" + name, required=True)
    parser.add_argument("--warmups", type=int, required=True)
    parser.add_argument("--samples", type=int, required=True)
    try:
        run_campaign(parser.parse_args())
    except (ValueError, OSError, ProbeInterrupted, KeyboardInterrupt) as error:
        raise SystemExit(str(error)) from error


if __name__ == "__main__":
    main()
