#!/usr/bin/env python3
"""Run the public BSN journey against `cargo package --workspace` artifacts.

Requires Python 3.12+ and a successful package command first. Only Router
dependencies are patched, to extracted tarballs rather than checkout sources.
"""
import json
import os
from pathlib import Path
import shutil
import subprocess
import tarfile
import tempfile


def cargo(*args, cwd, capture=False, env=None):
    return subprocess.run(
        ["cargo", *args], cwd=cwd, env=env, check=True,
        text=True, stdout=subprocess.PIPE if capture else None,
    ).stdout


def main():
    root = Path(__file__).resolve().parents[1]
    metadata = json.loads(cargo(
        "metadata", "--no-deps", "--offline", "--format-version", "1",
        cwd=root, capture=True,
    ))
    packages = [p for p in metadata["packages"] if p["id"] in metadata["workspace_members"]]
    rustc = subprocess.check_output(["rustc", "-vV"], text=True)
    host = next(line.removeprefix("host: ") for line in rustc.splitlines() if line.startswith("host: "))
    target = Path(metadata["target_directory"])
    env = {**os.environ, "CARGO_TARGET_DIR": str(target)}
    with tempfile.TemporaryDirectory(prefix="router-packaged-") as temporary:
        consumer = Path(temporary)
        extracted = {}
        for package in packages:
            name, version = package["name"], package["version"]
            archive = target / "package" / f"{name}-{version}.crate"
            with tarfile.open(archive) as source:
                source.extractall(consumer, filter="data")
            extracted[name] = consumer / f"{name}-{version}"
            # Cargo archives normalize mtimes. In a shared target, a changed
            # archive at the same package/version can otherwise look older than
            # a cached build. Recompile these path sources, keeping bytes and
            # registry dependency caches intact.
            for source_file in extracted[name].rglob("*"):
                if source_file.is_file():
                    os.utime(source_file, None)
        facade = next(p for p in packages if p["name"] == "bevy_ui_router")
        manifest = (
            '[package]\nname = "packaged-router-consumer"\nversion = "0.0.0"\nedition = "2024"\n'
            '[workspace]\nresolver = "3"\n[dependencies]\n'
            f'bevy_ui_router = {{ version = "={facade["version"]}", default-features = false, features = ["bsn", "platform"] }}\n'
        )
        for dependency in ("bevy_app", "bevy_asset", "bevy_ecs", "bevy_scene", "bevy_ui"):
            manifest += f'{dependency} = {{ version = "0.19", default-features = false }}\n'
        manifest += "[patch.crates-io]\n"
        for name, directory in extracted.items():
            manifest += f'{name} = {{ path = {json.dumps(str(directory))} }}\n'
        (consumer / "Cargo.toml").write_text(manifest, encoding="utf-8")
        (consumer / "src").mkdir()
        shutil.copyfile(
            extracted["bevy_ui_router"] / "tests/consumers/bsn.rs",
            consumer / "src/main.rs",
        )
        cargo("generate-lockfile", "--offline", cwd=consumer, env=env)
        resolved = json.loads(cargo(
            "metadata", "--offline", "--locked", "--filter-platform", host, "--format-version", "1",
            cwd=consumer, capture=True, env=env,
        ))
        for package in packages:
            matches = [p for p in resolved["packages"] if p["name"] == package["name"]]
            assert len(matches) == 1, package["name"]
            actual = matches[0]
            assert actual["version"] == package["version"], actual
            assert actual["source"] is None, actual
            assert Path(actual["manifest_path"]).resolve() == extracted[package["name"]] / "Cargo.toml", actual
        cargo("run", "--offline", "--locked", cwd=consumer, env=env)
        print(f"PASS: public BSN journey consumed all {len(packages)} extracted Router packages.")


if __name__ == "__main__":
    main()
