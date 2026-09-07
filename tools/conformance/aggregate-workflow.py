#!/usr/bin/env python3
"""Argument-vector Make bridge; environment values never become shell code."""
import os
import subprocess
import sys
from pathlib import Path


def required(name):
    value = os.environ.get(name)
    if not value:
        raise ValueError(f"{name} is required")
    return value


def main():
    mode = sys.argv[1]
    args = ["aggregate"]
    if mode == "summary":
        args += ["summary", "--lane", "normal-ci", "--check"]
    elif mode == "detail":
        args += ["detail", "--lane", required("LANE")]
    elif mode in ("baseline", "external-baseline"):
        args += ["baseline", "--lane", required("LANE"), "--external-evidence", "repository"]
        if mode == "external-baseline":
            args += ["--compare-dom-test", required("TEST_ID")]
    elif mode == "trend":
        args += ["trend"]
        for flag, name in (("--from-root", "FROM_ROOT"), ("--from", "FROM"),
                           ("--from-sha256", "FROM_SHA256"), ("--to-root", "TO_ROOT"),
                           ("--to", "TO"), ("--to-sha256", "TO_SHA256")):
            args += [flag, required(name)]
    else:
        raise ValueError("unknown aggregate workflow")
    command = ["cargo", "run", "-p", "conformance-runner", "--no-default-features",
               "--features", "aggregate", "--locked", "--", *args]
    return subprocess.run(command, cwd=Path(__file__).resolve().parents[2], check=False).returncode


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (ValueError, OSError) as error:
        print(f"aggregate workflow: {error}", file=sys.stderr)
        sys.exit(2)
