#!/usr/bin/env python3
"""Report release-content matches without including the matched values."""
import argparse
import collections
import json
from pathlib import Path
import re
import shutil
import subprocess

PATTERNS = {
    # Unix filesystem paths are case-sensitive. /users/42 is often a route.
    "home_path": re.compile(r'(?:/(?:home|Users)|/mnt/[a-z]/Users)/[^\s\"\'`<>]+|(?i:[A-Z]:[\\/]Users[\\/])[^\s\"\'`<>]+'),
    "email": re.compile(r'(?i)\b[A-Z0-9._%+\-]{1,64}@[A-Z0-9.\-]{1,253}\.[A-Z]{2,63}\b'),
    "private_key": re.compile(r'-----BEGIN (?:RSA |EC |OPENSSH |DSA |PGP )?PRIVATE KEY-----'),
    "credential_token": re.compile(r'\b(?:gh[pousr]_[A-Za-z0-9]{30,255}|github_pat_[A-Za-z0-9_]{50,255}|AKIA[0-9A-Z]{16}|ASIA[0-9A-Z]{16}|sk-(?:proj-|svcacct-)?[A-Za-z0-9_-]{20,255}|xox[baprs]-[A-Za-z0-9-]{20,255}|AIza[0-9A-Za-z_-]{35}|glpat-[A-Za-z0-9_-]{20,255})\b'),
    "url_credentials": re.compile(r'https?://[^\s/:]{1,128}:[^\s/@]{1,256}@'),
    "private_network": re.compile(r'(?<![\d.])(?:10\.(?:\d{1,3}\.){2}\d{1,3}|192\.168\.\d{1,3}\.\d{1,3}|172\.(?:1[6-9]|2\d|3[01])\.\d{1,3}\.\d{1,3})(?![\d.])'),
    "secret_assignment": re.compile(r'''(?i)\b(?:api[_-]?key|secret[_-]?key|access[_-]?token|password|passwd|client[_-]?secret)\b\s*[=:]\s*["']([^"'\n]{8,512})["']'''),
    "jwt": re.compile(r'\beyJ[A-Za-z0-9_-]{8,4096}\.[A-Za-z0-9_-]{8,4096}\.[A-Za-z0-9_-]{8,4096}\b'),
}


def git(*args):
    return subprocess.check_output(["git", *args])


def scan(data, patterns):
    text = data.decode("utf-8", errors="replace") if isinstance(data, bytes) else data
    hits = []
    for name, pattern in patterns.items():
        matches = list(pattern.finditer(text))
        if matches:
            hits.append({"category": name, "count": len(matches), "lines": sorted({text.count("\n", 0, m.start()) + 1 for m in matches})})
    return hits


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--history", action="store_true", help="Also scan every object reachable from local refs")
    parser.add_argument("--term-file", type=Path, help="UTF-8 newline-separated extra literal terms; keep local")
    parser.add_argument("--output", type=Path, required=True, help="JSON report path; contains locations, not matched values")
    args = parser.parse_args()
    patterns = dict(PATTERNS)
    if args.term_file:
        for i, term in enumerate(args.term_file.read_text(encoding="utf-8").splitlines(), 1):
            if term.strip():
                patterns[f"extra_term_{i}"] = re.compile(re.escape(term.strip()), re.I)
    root = Path(git("rev-parse", "--show-toplevel").decode().strip())
    if args.output.resolve().is_relative_to(root):
        parser.error("Write the audit report outside the checkout to avoid including it in a release")
    report = {"scope": "tracked and untracked, non-ignored files", "tools_available": {name: bool(shutil.which(name)) for name in ("gitleaks", "trufflehog", "detect-secrets")}, "files_scanned": 0, "bytes_scanned": 0, "findings": [], "history": None}
    files = sorted(set(git("-C", str(root), "ls-files", "--cached", "--others", "--exclude-standard", "-z").decode().split("\0")) - {""})
    for name in files:
        path = root / name
        if not path.is_file() or path.is_symlink():
            continue
        data = path.read_bytes()
        report["files_scanned"] += 1
        report["bytes_scanned"] += len(data)
        hits = scan(data, patterns)
        filename_hits = scan(name, patterns)
        if filename_hits:
            hits.extend({**hit, "location": "filename"} for hit in filename_hits)
        if hits:
            report["findings"].append({"path": name, "findings": hits})
    if args.history:
        history = {"shallow": git("rev-parse", "--is-shallow-repository").decode().strip() == "true", "object_counts": {}, "bytes": {}, "findings": [], "commit_identity": {}, "filename_findings": [], "ref_findings": []}
        counts, sizes, identities = collections.Counter(), collections.Counter(), collections.Counter()
        objects = [line.partition(" ") for line in git("rev-list", "--objects", "--all").decode().splitlines()]
        process = subprocess.Popen(["git", "cat-file", "--batch"], stdin=subprocess.PIPE, stdout=subprocess.PIPE)
        for oid, _, name in objects:
            process.stdin.write((oid + "\n").encode()); process.stdin.flush()
            header = process.stdout.readline().decode().split()
            if len(header) != 3:
                raise RuntimeError("Unable to read a reachable Git object")
            _, kind, size = header; size = int(size)
            data = process.stdout.read(size); process.stdout.read(1)
            counts[kind] += 1; sizes[kind] += size
            if kind in ("blob", "commit", "tag"):
                text = data.decode("utf-8", errors="replace")
                if kind in ("commit", "tag"):
                    for role, address in re.findall(r'^(author|committer|tagger) [^\n]* <([^>]+)>', text, re.M):
                        category = "github_noreply" if address.endswith("@users.noreply.github.com") else "other_email"
                        identities[f"{role}_{category}"] += 1
                    # Personal email in metadata remains a finding; GitHub noreply identities are counted separately.
                    text = re.sub(r'(?m)^((?:author|committer|tagger) [^\n]* <)[^>]+@users\.noreply\.github\.com(>)', r'\1REDACTED\2', text)
                hits = scan(text, patterns)
                if hits:
                    history["findings"].append({"object": oid, "kind": kind, "path": name, "findings": hits})
        process.stdin.close()
        if process.wait() != 0:
            raise RuntimeError("git cat-file failed")
        historical_paths = set()
        for commit in git("rev-list", "--all").decode().splitlines():
            historical_paths.update(git("ls-tree", "-r", "--name-only", "-z", commit).decode().split("\0"))
        historical_paths.discard("")
        history["unique_paths_scanned"] = len(historical_paths)
        for name in sorted(historical_paths):
            hits = scan(name, patterns)
            if hits:
                history["filename_findings"].append({"path": name, "findings": hits})
        for i, ref in enumerate(git("for-each-ref", "--format=%(refname)").decode().splitlines()):
            hits = scan(ref, patterns)
            if hits:
                history["ref_findings"].append({"ref_index": i, "findings": hits})
        history["object_counts"] = dict(counts); history["bytes"] = dict(sizes); history["commit_identity"] = dict(identities)
        report["history"] = history
    args.output.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"files_scanned": report["files_scanned"], "files_with_findings": len(report["findings"]), "history_objects": report["history"]["object_counts"] if report["history"] else None, "history_objects_with_findings": len(report["history"]["findings"]) if report["history"] else None, "report": str(args.output)}, ensure_ascii=False))


if __name__ == "__main__":
    main()
