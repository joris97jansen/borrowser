#!/usr/bin/env python3
"""Read-only official upstream observations for Borrowser's rust-url and ureq."""

import sys

if __name__ == "__main__" and (not sys.flags.isolated or not sys.dont_write_bytecode):
    print("Run this qualification entry point with python3 -I -B (isolated imports, no cache writes).",
          file=sys.stderr)
    sys.exit(2)

from datetime import datetime, timezone
import html
import json
import os
from pathlib import Path
import re
import signal
import subprocess
import tempfile
import unicodedata

if __name__ == "__main__":
    if subprocess.run([sys.executable, "-I", "-B", str(Path(__file__).with_name(
            "check_dependency_subtrees.py")), "--python-preflight"]).returncode:
        sys.exit(2)
    sys.path.insert(0, str(Path(__file__).resolve().parent))

import check_dependency_subtrees as subtrees
import check_url_dependencies as url_policy
import check_ureq_patch as ureq_policy

UPSTREAM = "https://github.com/servo/rust-url.git"
UREQ_UPSTREAM = "https://github.com/algesten/ureq.git"
REVISION = re.compile(r"[0-9a-f]{40}")
EXIT_CODES = {"notice": 0, "warning": 1, "error": 2}


class GitError(RuntimeError):
    """A Git operation failed or could not finish within its deadline."""


def git(repo, *args, timeout=15, allowed=(0,)):
    """Capture all output and kill the whole Git/helper process group on timeout."""
    command = ["git", "-c", "credential.helper=", "-c", "core.hooksPath=/dev/null",
               "-C", str(repo), *args]
    environment = dict(os.environ, GIT_TERMINAL_PROMPT="0", LC_ALL="C")
    try:
        with subprocess.Popen(command, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                              text=True, encoding="utf-8", errors="replace",
                              start_new_session=True, env=environment) as process:
            try:
                stdout, stderr = process.communicate(timeout=timeout)
            except subprocess.TimeoutExpired:
                try:
                    os.killpg(process.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
                process.communicate()
                raise GitError(f"git {args[0]} timed out after {timeout} seconds") from None
            if process.returncode not in allowed:
                raise GitError(f"git {args[0]} exited {process.returncode}: {stderr[:4096]}")
            return subprocess.CompletedProcess(command, process.returncode, stdout, stderr)
    except OSError as error:
        raise GitError(f"git {args[0]} unavailable: {error}") from error


def escaped(value, limit=4096):
    """Render external text only in HTML blocks, with visible control characters."""
    text = str(value)
    text = "".join(c if c in "\n\t" or unicodedata.category(c) not in {"Cc", "Cf", "Zl", "Zp"}
                   else (f"\\x{ord(c):02x}" if ord(c) < 256 else f"\\u{ord(c):04x}") for c in text)
    if len(text) > limit:
        text = text[:limit] + " [truncated]"
    return html.escape(text)


def compare(repo, left, right):
    """Classify reachability, preserving errors separately from false ancestry."""
    counts = git(repo, "rev-list", "--left-right", "--count", f"{left}...{right}").stdout.split()
    left_count, right_count = map(int, counts)
    forward = git(repo, "merge-base", "--is-ancestor", left, right, allowed=(0, 1)).returncode == 0
    reverse = git(repo, "merge-base", "--is-ancestor", right, left, allowed=(0, 1)).returncode == 0
    bases = git(repo, "merge-base", "--all", left, right, allowed=(0, 1)).stdout.split()
    if forward and reverse:
        state = "equal"
    elif forward:
        state = "left-ancestor"
    elif reverse:
        state = "right-ancestor"
    else:
        state = "divergent" if bases else "unrelated"
    return state, left_count, right_count, bases


def history(repo, base, head, count, repository):
    if not count:
        return []
    lines = [f"Showing up to 20 of {count} commits absent from the other side's ancestry:", ""]
    log = git(repo, "log", "-z", "--max-count=20", "--format=%H%x00%s", f"{base}..{head}").stdout
    entries = log.split("\0")
    if entries.pop() != "" or len(entries) % 2:
        raise GitError("invalid commit records in Git log")
    for sha, subject in zip(entries[::2], entries[1::2]):
        if REVISION.fullmatch(sha) is None:
            raise GitError("invalid commit identity in Git log")
        lines.extend([f"- [{sha}]({repository.removesuffix('.git')}/commit/{sha})", "",
                      f"<pre>{escaped(subject, 512)}</pre>", ""])
    if count > 20:
        lines.append(f"Commit list truncated; {count - 20} additional commits are not shown.")
    return lines


def observe(root, name):
    # Two fixed dependencies, not a configurable monitoring framework.
    repository, branch = ((UPSTREAM, "main") if name == "rust-url" else
                          (UREQ_UPSTREAM, "2.12.x-msrv1.71"))
    findings, lines = [], [f"## {name}", ""]
    policy = None

    def finding(level, message):
        findings.append((level, f"{name}: {message}"))
        lines.extend([f"**{level.upper()}: {message}**", ""])

    try:
        policy = subtrees.load_qualification(root)
        subtrees.check_repo(root)
        if name == "rust-url":
            for label in ("main", "fuzz"):
                url_policy.check_workspace_records(root, label)
        else:
            ureq_policy.check_workspace_records(root)
    except (OSError, ValueError, KeyError, TypeError, subprocess.SubprocessError) as error:
        finding("error", "Unqualified local source, history or consumer configuration.")
        lines.append(f"<pre>{escaped(error)}</pre>")

    try:
        with tempfile.TemporaryDirectory(prefix=f"{name}-upstream-", dir=os.environ.get("RUNNER_TEMP")) as temporary:
            repo = Path(temporary)
            git(repo, "init", "--bare")
            git(repo, "fetch", "--no-tags", "--no-recurse-submodules", repository,
                f"refs/heads/{branch}:refs/remotes/monitor/upstream", timeout=60)
            tip = git(repo, "rev-parse", "--verify", "refs/remotes/monitor/upstream^{commit}").stdout.strip()
            lines.extend([f"Official `{branch}`: `{tip}`", ""])
            if policy is None:
                finding("error", "Qualified baseline unavailable; comparison incomplete.")
            else:
                baseline = policy["dependencies"][name]["upstream_revision"]
                git(repo, "fetch", "--no-tags", "--no-recurse-submodules", str(root), baseline)
                state, left, right, _ = compare(repo, baseline, tip)
                lines.extend([f"Qualified official baseline: `{baseline}`", "",
                              f"Relationship: **{state}**; baseline-only: {left}; upstream-only: {right}.", ""])
                if state == "equal":
                    lines.extend(["**Current.** No upstream changes.", ""])
                elif state == "left-ancestor":
                    finding("warning", "Review available; upstream changes are not adopted.")
                    lines.extend(history(repo, baseline, tip, right, repository))
                else:
                    finding("error", "Unexpected upstream divergence or rewind; manual review required.")
    except (GitError, OSError, ValueError, KeyError, TypeError) as error:
        finding("error", "Upstream monitoring failed or is incomplete.")
        lines.append(f"<pre>{escaped(error)}</pre>")
    return lines, findings


def monitor(root):
    lines = ["# rust-url and ureq synchronization", "",
             f"Observed: {datetime.now(timezone.utc).isoformat()}", ""]
    findings = []
    for name in ("rust-url", "ureq"):
        # A failure must never suppress the other independent observation.
        try:
            dependency_lines, dependency_findings = observe(root, name)
        except Exception as error:
            dependency_lines = [f"## {name}", f"<pre>{escaped(error)}</pre>"]
            dependency_findings = [("error", f"{name}: observation incomplete.")]
        lines.extend(dependency_lines)
        findings.extend(dependency_findings)
    code = max((EXIT_CODES[level] for level, _ in findings), default=0)
    result = {0: "Current", 1: "Review available", 2: "Error / incomplete / unqualified"}[code]
    lines.extend([f"**Result: {result} (exit {code}).**", "",
                  "Read-only observation; source qualification and owner authorization are separate."])
    return code, "\n".join(lines) + "\n", findings


def main():
    try:
        code, summary, findings = monitor(Path(__file__).resolve().parents[2])
    except Exception as error:
        code = 2
        summary = "# rust-url and ureq synchronization\n\n**Monitoring incomplete.**\n" + f"<pre>{escaped(error)}</pre>\n"
        findings = [("error", "Dependency observations incomplete.")]
    try:
        if os.environ.get("GITHUB_STEP_SUMMARY"):
            with open(os.environ["GITHUB_STEP_SUMMARY"], "a", encoding="utf-8") as output:
                output.write(summary)
        else:
            print("rust-url and ureq summary: " + json.dumps(summary))
    except OSError:
        print("::error::Could not write dependency synchronization summary.")
        return 2
    for level, message in findings:
        print(f"::{level}::{message}")
    print(f"rust-url and ureq synchronization exit code: {code}")
    return code


if __name__ == "__main__":
    sys.exit(main())
