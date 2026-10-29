#!/usr/bin/env python3
"""Observe rust-url ancestry and recorded consumer pins; never qualify or update them."""

from datetime import datetime, timezone
import html
import json
import os
from pathlib import Path
import re
import signal
import subprocess
import sys
import tempfile
import tomllib
import unicodedata

UPSTREAM = "https://github.com/servo/rust-url.git"
FORK = "https://github.com/joris97jansen/rust-url.git"
REGISTRY = "registry+https://github.com/rust-lang/crates.io-index"
FORK_PACKAGES = {"url", "idna", "form_urlencoded", "percent-encoding"}
REVISION = re.compile(r"[0-9a-f]{40}")
LOCK_SOURCE = re.compile(
    r"git\+https://github\.com/joris97jansen/rust-url(?:\.git)?"
    r"\?rev=([0-9a-f]{40})#([0-9a-f]{40})"
)
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


def workspace_pin(manifest, lock):
    """Read only the existing Cargo patch form; Cargo CI owns actual resolution."""
    patches = manifest.get("patch", {}).get("crates-io", {})
    for dependencies in (manifest.get("dependencies", {}),
                         manifest.get("workspace", {}).get("dependencies", {})):
        declaration = dependencies.get("url")
        if declaration is not None and not isinstance(declaration, (str, dict)):
            raise ValueError("url dependency declaration must be a version string or table")
        if isinstance(declaration, dict) and ({"git", "path", "rev", "branch", "tag", "registry"} & declaration.keys()
                                               or declaration.get("package", "url") != "url"):
            raise ValueError("url source overrides outside [patch.crates-io] are unsupported")
    packages = lock.get("package", [])
    urls = [p for p in packages if p.get("name") == "url"]
    if len(urls) != 1:
        raise ValueError("expected exactly one locked url package")
    patch = patches.get("url")
    if patch is None:
        if urls[0].get("source") != REGISTRY:
            raise ValueError("url has no fork patch but its lock source is not crates.io")
        for name in patches:
            if name in FORK_PACKAGES:
                raise ValueError("partial or unsupported rust-url patch configuration")
        if any(p.get("name") in FORK_PACKAGES and p.get("source") != REGISTRY for p in packages):
            raise ValueError("partial or unsupported rust-url lock configuration")
        return None
    if not isinstance(patch, dict) or not isinstance(patch.get("rev"), str):
        raise ValueError("url patch requires a full Git revision")
    pin = patch["rev"]
    if REVISION.fullmatch(pin) is None:
        raise ValueError("url patch requires a full lowercase 40-character Git revision")
    for name in FORK_PACKAGES & patches.keys():
        entry = patches[name]
        if (not isinstance(entry, dict) or entry.get("git") not in (FORK, FORK.removesuffix(".git"))
                or entry.get("rev") != pin
                or {"branch", "tag", "path", "registry"} & entry.keys()
                or entry.get("package", name) != name):
            raise ValueError("conflicting or unsupported rust-url patch declaration")
    for package in packages:
        source = package.get("source", "")
        if package.get("name") in FORK_PACKAGES or "github.com/joris97jansen/rust-url" in source:
            match = LOCK_SOURCE.fullmatch(source)
            if match is None or match.groups() != (pin, pin):
                raise ValueError("rust-url manifest and locked sources must use the same revision")
    return pin


def consumer_pin(root):
    pins = []
    for directory in (root, root / "fuzz"):
        try:
            manifest = tomllib.loads((directory / "Cargo.toml").read_text())
            lock = tomllib.loads((directory / "Cargo.lock").read_text())
            pins.append(workspace_pin(manifest, lock))
        except (OSError, ValueError, TypeError, AttributeError, KeyError) as error:
            label = "main" if directory == root else "fuzz"
            raise ValueError(f"{label} consumer configuration: {error}") from error
    if pins[0] != pins[1]:
        raise ValueError("main and fuzz workspaces have conflicting rust-url adoption or revisions")
    return pins[0]


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


def monitor(root):
    lines = [f"Observed: {datetime.now(timezone.utc).isoformat()}", ""]
    findings = []

    def finding(level, message):
        # Callers supply only controlled messages, never Git output or commit text.
        findings.append((level, message))
        lines.extend([f"**{level.upper()}: {message}**", ""])

    try:
        checkout = git(root, "rev-parse", "--verify", "HEAD^{commit}").stdout.strip()
        lines.extend([f"Borrowser checkout: `{checkout}`", "",
                      f"<pre>Ref: {escaped(os.environ.get('GITHUB_REF', 'local checkout'))}</pre>", ""])
    except GitError as error:
        finding("error", "Borrowser checkout identity unavailable.")
        lines.append(f"<pre>{escaped(error)}</pre>")

    pin = None
    configured = False
    try:
        pin = consumer_pin(root)
        configured = True
        lines.extend([f"Main/fuzz consumer pin: [{pin}]({FORK.removesuffix('.git')}/commit/{pin})"
                      if pin else "Consumer: not adopted (crates.io).", ""])
    except ValueError as error:
        finding("error", "Consumer configuration error.")
        lines.append(f"<pre>{escaped(error)}</pre>")

    try:
        with tempfile.TemporaryDirectory(prefix="rust-url-sync-", dir=os.environ.get("RUNNER_TEMP")) as temporary:
            repo = Path(temporary)
            git(repo, "init", "--bare")
            refs = {}
            for name, url, branch in (("upstream", UPSTREAM, "main"),
                                      ("maintained", FORK, "borrowser"), ("mirror", FORK, "main")):
                ref = f"refs/remotes/monitor/{name}"
                try:
                    git(repo, "fetch", "--no-tags", "--no-recurse-submodules", url,
                        f"refs/heads/{branch}:{ref}", timeout=60)
                    sha = git(repo, "rev-parse", "--verify", f"{ref}^{{commit}}").stdout.strip()
                    refs[name] = sha
                    lines.extend([f"{name} ({branch}): [{sha}]({url.removesuffix('.git')}/commit/{sha})", ""])
                except GitError as error:
                    finding("error", f"The {name} reference is unavailable.")
                    lines.append(f"<pre>{escaped(error)}</pre>")

            def inspect(title, left, right, left_url, right_url):
                lines.extend([f"### {title}", ""])
                if left is None or right is None:
                    lines.extend(["Not compared: a required reference is unavailable.", ""])
                    return None
                try:
                    state, left_count, right_count, bases = compare(repo, left, right)
                    lines.extend([f"Relationship: **{state}**. Left-only: **{left_count}**; right-only: **{right_count}**.", "",
                                  "Common ancestors: " + (", ".join(f"`{base}`" for base in bases) or "none"), ""])
                    if left_count:
                        lines.extend(["**Left-only commits:**", ""])
                        lines.extend(history(repo, right, left, left_count, left_url))
                    if right_count:
                        lines.extend(["**Right-only commits:**", ""])
                        lines.extend(history(repo, left, right, right_count, right_url))
                    if state == "unrelated":
                        finding("error", "Unrelated histories; synchronization cannot be established.")
                    return state
                except (GitError, ValueError) as error:
                    finding("error", "Git comparison could not complete.")
                    lines.append(f"<pre>{escaped(error)}</pre>")
                    return None

            state = inspect("Upstream (left) / maintained fork (right)", refs.get("upstream"), refs.get("maintained"), UPSTREAM, FORK)
            if state in ("right-ancestor", "divergent"):
                finding("warning", "Upstream commits are absent from maintained-fork ancestry; synchronization review needed.")
            elif state in ("equal", "left-ancestor"):
                lines.extend(["Upstream ancestry incorporated; fork-only commits are permitted.", ""])
            lines.extend(["[Live upstream comparison](https://github.com/servo/rust-url/compare/joris97jansen:borrowser...main)", ""])

            state = inspect("Upstream (left) / fork main mirror (right)", refs.get("upstream"), refs.get("mirror"), UPSTREAM, FORK)
            if state in ("left-ancestor", "right-ancestor", "divergent"):
                finding("warning", "Fork main differs from upstream main; mirror review needed.")
            elif state == "equal":
                lines.extend(["Fork main matches upstream main.", ""])
            lines.extend(["[Live mirror comparison](https://github.com/servo/rust-url/compare/joris97jansen:main...main)", ""])

            if configured and pin:
                try:
                    if git(repo, "cat-file", "-e", f"{pin}^{{commit}}", allowed=(0, 1, 128)).returncode:
                        git(repo, "fetch", "--no-tags", "--no-recurse-submodules", FORK, pin, timeout=60)
                    git(repo, "rev-parse", "--verify", f"{pin}^{{commit}}")
                except GitError as error:
                    finding("error", "Consumer pin is unavailable for comparison.")
                    lines.append(f"<pre>{escaped(error)}</pre>")
                else:
                    state = inspect("Consumer pin (left) / maintained fork (right)", pin, refs.get("maintained"), FORK, FORK)
                    if state == "left-ancestor":
                        finding("notice", "New maintained-fork revision available; qualification required before adoption.")
                    elif state in ("right-ancestor", "divergent"):
                        finding("warning", "Consumer ancestry is ahead of or divergent from the maintained branch; manual review needed.")
                    elif state == "equal":
                        lines.extend(["Consumer pin matches the maintained branch.", ""])
                    if refs.get("maintained"):
                        lines.append(f"[Consumer comparison](https://github.com/joris97jansen/rust-url/compare/{pin}...{refs['maintained']})")
    except (GitError, OSError) as error:
        finding("error", "Monitor could not complete.")
        lines.append(f"<pre>{escaped(error)}</pre>")

    code = max((EXIT_CODES[level] for level, _ in findings), default=0)
    result = {0: "No synchronization warning (availability notices may apply)",
              1: "Synchronization or history review needed", 2: "Configuration error or monitoring incomplete"}[code]
    summary = "# rust-url synchronization\n\n**Result: " + result + "**\n\n" + "\n".join(lines) + "\n"
    return code, summary, findings


def main():
    try:
        code, summary, findings = monitor(Path(__file__).resolve().parents[2])
    except Exception as error:
        code = 2
        summary = f"# rust-url synchronization\n\n**Monitoring incomplete.**\n\n<pre>{escaped(error)}</pre>\n"
        findings = [("error", "Monitor could not complete.")]
    try:
        if os.environ.get("GITHUB_STEP_SUMMARY"):
            with open(os.environ["GITHUB_STEP_SUMMARY"], "a", encoding="utf-8") as output:
                output.write(summary)
        else:
            # One encoded line prevents external text from becoming Actions commands.
            print("rust-url summary: " + json.dumps(summary))
    except OSError:
        print("::error::Could not write the rust-url synchronization summary.")
        return 2
    for level, message in findings:
        print(f"::{level}::{message}")
    print(f"rust-url synchronization exit code: {code}")
    return code


if __name__ == "__main__":
    sys.exit(main())
