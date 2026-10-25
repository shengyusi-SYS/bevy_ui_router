#!/usr/bin/env python3
"""Check local Markdown links, language pairs, code examples, and package licenses."""
from collections import Counter
from pathlib import Path
import re
import sys
import tomllib
from urllib.parse import unquote, urlsplit

ROOT = Path(__file__).resolve().parents[1]
REPOSITORY = "https://github.com/shengyusi-SYS/bevy_ui_router/blob/main/"


def outside_fences(text):
    lines, fence = [], None
    for line in text.splitlines():
        match = re.match(r"^\s*(`{3,}|~{3,})", line)
        if match:
            marker = match[1][0]
            fence = None if fence == marker else marker if fence is None else fence
            lines.append("")
        else:
            lines.append(line if fence is None else "")
    return "\n".join(lines)


def anchors(path):
    text = outside_fences(path.read_text(encoding="utf-8"))
    result = set(re.findall(r'<a\s+(?:id|name)=["\']([^"\']+)', text))
    counts = Counter()
    for heading in re.findall(r"^#{1,6}\s+(.+?)\s*#*\s*$", text, re.M):
        heading = re.sub(r"\[([^\]]+)\]\([^)]*\)", r"\1", heading)
        heading = re.sub(r"<[^>]*>", "", heading)
        slug = "".join(c for c in heading.lower() if c.isalnum() or c in " _-").replace(" ", "-")
        duplicate = counts[slug]
        counts[slug] += 1
        result.add(slug if not duplicate else f"{slug}-{duplicate}")
    return result


def examples(text):
    return re.findall(r"^```(sh|toml|json)\s*\n(.*?)^```", text, re.M | re.S)


def main():
    errors = []
    paths = [ROOT / "AGENTS.md", ROOT / "README.md", ROOT / "README.en.md",
             ROOT / "CONTRIBUTING.md", ROOT / "CONTRIBUTING.en.md", *sorted((ROOT / "docs").rglob("*.md"))]
    anchor_cache = {}
    links = 0
    for path in paths:
        if not path.is_file():
            errors.append(f"Missing documentation: {path.relative_to(ROOT)}")
            continue
        text = path.read_text(encoding="utf-8")
        for target in re.findall(r"!?\[[^\]\n]*\]\(([^)\n]+)\)", outside_fences(text)):
            target = target.strip().strip("<>")
            if target.startswith(REPOSITORY):
                target = str(ROOT / target[len(REPOSITORY):])
            parsed = urlsplit(target)
            if parsed.scheme or parsed.netloc:
                continue
            destination = (path.parent / unquote(parsed.path)).resolve() if parsed.path else path
            links += 1
            if not destination.exists():
                errors.append(f"{path.relative_to(ROOT)}: missing link {target}")
            elif parsed.fragment and destination.suffix == ".md":
                if destination not in anchor_cache:
                    anchor_cache[destination] = anchors(destination)
                if unquote(parsed.fragment) not in anchor_cache[destination]:
                    errors.append(f"{path.relative_to(ROOT)}: missing anchor {target}")
        relative = path.relative_to(ROOT)
        if path.name != "AGENTS.md" and not path.name.endswith(".en.md"):
            english = path.with_name(path.stem + ".en.md")
            if not english.is_file():
                errors.append(f"{relative}: missing English version")
            elif examples(text) != examples(english.read_text(encoding="utf-8")):
                errors.append(f"{relative}: shell/TOML/JSON examples differ between languages")
    workspace = tomllib.loads((ROOT / "Cargo.toml").read_text())
    if workspace["workspace"]["package"].get("license") != "MIT OR Apache-2.0":
        errors.append("Workspace must declare MIT OR Apache-2.0")
    for member in workspace["workspace"]["members"]:
        manifest = tomllib.loads((ROOT / member / "Cargo.toml").read_text())
        if manifest["package"].get("license") != {"workspace": True}:
            errors.append(f"{member}: license must inherit the workspace value")
        for name in ("LICENSE-MIT", "LICENSE-APACHE"):
            license_path = ROOT / member / name
            if not license_path.is_file() or license_path.read_bytes() != (ROOT / name).read_bytes():
                errors.append(f"{member}/{name}: differs from the root license")
    for error in errors:
        print(error, file=sys.stderr)
    if errors:
        return 1
    print(f"PASS: {len(paths)} Markdown files, {links} local links, bilingual examples, and six package license pairs.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
