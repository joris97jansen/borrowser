#!/usr/bin/env python3
"""AJ8's five release cases using the unchanged concrete AJ5 supervisor."""
import argparse
from types import ModuleType
import json
from pathlib import Path
import signal
import socket
import subprocess

# Execute the approved source directly: -B does not prohibit reading an existing
# bytecode cache beside the supervisor. Leave any owner's cache untouched.
_supervisor_source = Path(__file__).with_name("aj5_diagnostics_probe.py")
aj5 = ModuleType("aj5_supervisor")
aj5.__file__ = str(_supervisor_source)
exec(compile(_supervisor_source.read_bytes(), str(_supervisor_source), "exec"), aj5.__dict__)
CASES = ("", "before-navigation", "after-navigation", "after-first-terminal", "abort")

def accepted(result, mode, closed, unchanged):
    cleanup = result["cleanup"]
    expected_failure = "exit-status" if mode else None
    clean = (cleanup.get("verified_absent") is True and cleanup.get("failure") is None
             and closed and unchanged and result["failure"] == expected_failure)
    if not clean:
        return False
    if mode == "abort":
        return result["exit"] == -signal.SIGABRT and "AJ8 controlled optimized-abort fixture" in result["stderr"]
    return (result["exit"] == (1 if mode else 0)
            and "server=Ok(()) runtime=Ok(()) cleanup_verified=true" in result["stdout"]
            and ('primary=Err("injected loopback failure")' if mode else "primary=Ok(())") in result["stdout"]
            and (bool(mode) or "AJ8_SEMANTIC_PASS positions=11 actual_charset=transport redirected_base=true empty_parse=true style_execution=false" in result["stdout"]))

def qualify(binary, evidence):
    evidence.mkdir(parents=True, exist_ok=False)
    identity = aj5.digest(binary)
    source = {"head": subprocess.check_output(["git", "rev-parse", "HEAD"], text=True).strip(),
              "tree": subprocess.check_output(["git", "rev-parse", "HEAD^{tree}"], text=True).strip(),
              "fingerprint": aj5.source_fingerprint(Path.cwd()), "binary_sha256": identity}
    (evidence / "identity.json").write_text(json.dumps(source, indent=2) + "\n")
    for mode in CASES:
        if aj5.digest(binary) != identity:
            raise ValueError("AJ8 binary drift before execution")
        result = aj5.supervise([str(binary)] + ([mode] if mode else []), inputs={},
                               process_seconds=70, case_seconds=70, output_limit=65536, measurement_records=False)
        markers = [line.split()[1] for line in result["stdout"].splitlines() if line.startswith("AJ8_SMOKE_SERVER ")]
        closed = False
        if len(markers) == 1:
            host, port = markers[0].rsplit(":", 1)
            if host == "127.0.0.1" and 0 < int(port) < 65536:
                try:
                    with socket.create_connection((host, int(port)), timeout=.2):
                        pass
                except ConnectionRefusedError:
                    closed = True
        unchanged = aj5.digest(binary) == identity
        result.update(mode=mode or "success", listener_verified_closed=closed,
                      executable_unchanged=unchanged, executable_sha256=identity)
        result["qualification_passed"] = accepted(result, mode, closed, unchanged)
        (evidence / ((mode or "success") + ".json")).write_text(json.dumps(result, indent=2) + "\n")
        if not result["qualification_passed"]:
            raise ValueError(f"AJ8 {mode or 'success'} failed; no retry: {result['failure']}")
    if source["fingerprint"] != aj5.source_fingerprint(Path.cwd()):
        raise ValueError("AJ8 source drift during campaign")

if __name__ == "__main__":
    parser = argparse.ArgumentParser(__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--evidence", type=Path, required=True)
    args = parser.parse_args()
    qualify(args.binary.resolve(), args.evidence)
