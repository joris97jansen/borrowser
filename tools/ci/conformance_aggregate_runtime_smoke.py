#!/usr/bin/env python3
"""Linux-only, fail-closed runtime proof for AG9e normal-ci summary execution."""
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile


def tool(name):
    path = shutil.which(name)
    if path is None:
        raise RuntimeError(f"required isolation tool unavailable: {name}")
    return path


def verify_trace(records, binary):
    """Audit this command only. Linux threads are not helper processes."""
    launches = re.findall(r'\bexecve(?:at)?\(', records)
    # strace -xx renders OS-native executable bytes unambiguously, including
    # whitespace/quotes/non-UTF-8. Require the exact prepared executable.
    encoded = "".join(f"\\x{byte:02x}" for byte in os.fsencode(binary))
    initial = re.search(r'\bexecve\("' + re.escape(encoded) + r'",.*= 0$', records, re.MULTILINE)
    if len(launches) != 1 or initial is None:
        raise RuntimeError("trace must contain exactly the expected initial executable launch")
    for call in re.finditer(r'\b(clone3?|fork|vfork)\(([^\n]*)', records):
        name, arguments = call.groups()
        # Reject fork-like clones, but permit proven CLONE_THREAD calls,
        # including calls strace splits into unfinished/resumed records.
        if name in ("fork", "vfork") or not re.search(r'\bflags=[^,}\n]*\bCLONE_THREAD\b', arguments):
            raise RuntimeError("AG9e summary attempted non-thread process creation")
    if re.search(r"\bAF_INET6?\b|\b(?:connect|sendto|sendmsg|sendmmsg)\(", records):
        raise RuntimeError("AG9e summary attempted network communication")


def main():
    if sys.platform != "linux":
        raise RuntimeError("AG9e runtime isolation requires Linux network namespaces and strace")
    binary = Path(sys.argv[1]).resolve(strict=True)
    arguments = [str(binary), "aggregate", "summary", "--lane", "normal-ci", "--check"]
    reference = subprocess.run(arguments, capture_output=True, check=True)
    with tempfile.TemporaryDirectory(prefix="ag9e-runtime-") as directory:
        trace = Path(directory) / "trace"
        # env is outside the trace. Only the prepared runner and its descendants
        # are traced. Cargo, tool installation, and setup are outside this proof.
        command = [tool("sudo"), "-n", tool("unshare"), "--net", "--",
                   tool("env"), "-i", "PATH=/nonexistent",
                   tool("strace"), "-f", "-qq", "-xx", "-s", "4096", "-e", "trace=network,process",
                   "-o", str(trace), "--", *arguments]
        result = subprocess.run(command, capture_output=True, check=False)
        if result.returncode != 0:
            raise RuntimeError(f"isolated summary failed ({result.returncode}): {result.stderr.decode(errors='replace')}")
        if result.stderr or result.stdout != reference.stdout or len(result.stdout) > 6073:
            raise RuntimeError("isolated summary output differs or exceeds the reviewed bound")
        records = trace.read_text()
        verify_trace(records, binary)
    print("AG9e isolated summary: identical bounded bytes; no network communication or helper execution (threads permitted)", file=sys.stderr)


if __name__ == "__main__":
    try:
        main()
    except (OSError, RuntimeError, subprocess.SubprocessError) as error:
        print(f"AG9e runtime proof failed: {error}", file=sys.stderr)
        sys.exit(1)
