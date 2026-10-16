#!/usr/bin/env python3
"""Reconstruct the isolated native-input candidate without changing dependencies."""

import argparse
import hashlib
import io
import json
import os
from pathlib import Path
import shutil
import subprocess
import tarfile
import tempfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--prepare-only", action="store_true")
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    assets = root / "scripts/native-input-candidate"
    sources = json.loads((assets / "sources.json").read_text())
    cargo_home = Path(os.environ.get("CARGO_HOME", Path.home() / ".cargo"))
    registry = cargo_home / "registry/src"
    originals = {}
    for package in ("bevy_input_focus", "bevy_ui"):
        candidates = sorted(registry.glob(f"*/{package}-{sources['bevy_version']}"))
        for candidate in candidates:
            expected = sources["original_ui_sha256"]
            if all(
                hashlib.sha256((candidate / name.split("/", 1)[1]).read_bytes()).hexdigest()
                == digest
                for name, digest in expected.items()
                if name.startswith(package + "/")
            ):
                originals[package] = candidate
                break
        else:
            raise SystemExit(f"Expected cached official source not found: {package}")

    (root / "target").mkdir(exist_ok=True)
    output = Path(tempfile.mkdtemp(prefix="native-input-candidate-", dir=root / "target"))
    probe = output / "input-seam-probe"
    router = output / "input-seam-router"
    probe.mkdir()
    router.mkdir()
    archive = subprocess.check_output(["git", "archive", sources["base_commit"]], cwd=root)
    with tarfile.open(fileobj=io.BytesIO(archive)) as files:
        files.extractall(router, filter="data")
    for package, original in originals.items():
        shutil.copytree(original, probe / package)
    for filename in ("Cargo.toml", "Cargo.lock"):
        shutil.copyfile(assets / filename, probe / filename)
    shutil.copytree(assets / "tests", probe / "tests")
    for patch, destination in (("ui.patch", probe), ("router.patch", router)):
        subprocess.run(
            ["git", "apply", "--whitespace=error", "--directory=" + str(destination.relative_to(root)),
             str(assets / patch)], cwd=root, check=True,
        )
    print(f"Candidate: {output}", flush=True)
    if args.prepare_only:
        return

    checks = [
        ("format", ["cargo", "fmt", "--manifest-path", str(router / "Cargo.toml"), "--all", "--check"]),
        ("ui", ["cargo", "test", "--offline", "--locked", "--tests", "--manifest-path", str(probe / "Cargo.toml"),
                "-p", "bevy_input_focus", "-p", "bevy-router-input-seam-probe"]),
        ("ecs", ["cargo", "test", "--offline", "--locked", "--manifest-path", str(router / "Cargo.toml"),
                 "-p", "bevy_ui_router_ecs", "--all-features"]),
        ("public", ["cargo", "test", "--offline", "--locked", "--manifest-path", str(router / "Cargo.toml"),
                    "-p", "bevy_ui_router", "--all-features"] + [
                        argument for name in ("bsn_native_focus", "bsn_native_input", "bsn_native_navigation",
                                              "public_activity", "retained_navigation", "scroll_restoration",
                                              "public_navigation_journeys") for argument in ("--test", name)
                    ]),
    ]
    for name, command in checks:
        if name != "format":
            command += ["--target-dir", str(root / "target")]
        log = output / f"{name}.log"
        print(f"Checking {name}: {log}", flush=True)
        with log.open("w") as stream:
            result = subprocess.run(command, cwd=root, stdout=stream, stderr=subprocess.STDOUT)
        if result.returncode:
            print("\n".join(log.read_text().splitlines()[-50:]))
            raise SystemExit(result.returncode)
        print(f"Passed {name}", flush=True)


if __name__ == "__main__":
    main()
