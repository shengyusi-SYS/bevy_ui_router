#!/usr/bin/env python3
"""Drive router_windowed's debug-only BRP endpoint without touching OS input/focus.

Run the example with --features ui-debug, then use snapshot/find to locate nodes.
click uses logical window coordinates; divide center_physical by scale_factor.
For a held gesture, rpc router/input accepts separate MouseButtonInput Pressed
and Released WindowEvent values. Snapshot polling observes ordinary frame progress.
"""

import argparse
import json
import pathlib
import sys
import time
import urllib.request


class Bridge:
    def __init__(self, url):
        self.url = url

    def rpc(self, method, params=None):
        body = {"jsonrpc": "2.0", "id": 1, "method": method}
        if params is not None:
            body["params"] = params
        request = urllib.request.Request(
            self.url, json.dumps(body).encode(), {"Content-Type": "application/json"}
        )
        with urllib.request.urlopen(request, timeout=10) as response:
            result = json.load(response)
        if "error" in result:
            raise RuntimeError(json.dumps(result["error"], ensure_ascii=False))
        return result["result"]

    def snapshot(self):
        return self.rpc("router/ui_snapshot")

    def frames(self, count=2):
        start = self.snapshot()["frame"]
        deadline = time.monotonic() + 10
        while time.monotonic() < deadline:
            if (self.snapshot()["frame"] - start) % (2**32) >= count:
                return
            time.sleep(0.02)
        raise TimeoutError("Bevy did not advance frames within 10 seconds")

    def window(self):
        windows = self.snapshot()["windows"]
        if len(windows) != 1:
            raise RuntimeError("Expected exactly one primary window")
        return windows[0]

    def event(self, variant, value):
        return self.rpc("router/input", {variant: value})

    def move(self, x, y):
        self.event("CursorMoved", {
            "window": self.window()["entity"], "position": [x, y], "delta": None
        })
        self.frames()

    def click(self, x, y):
        self.move(x, y)
        window = self.window()["entity"]
        self.event("MouseButtonInput", {"window": window, "button": "Left", "state": "Pressed"})
        try:
            self.frames()
        finally:
            self.event("MouseButtonInput", {"window": window, "button": "Left", "state": "Released"})
        self.frames()

    def key(self, code, logical):
        value = {"window": self.window()["entity"], "key_code": code,
                 "logical_key": logical, "repeat": False,
                 "text": logical.get("Character") if isinstance(logical, dict) else None}
        self.event("KeyboardInput", {**value, "state": "Pressed"})
        try:
            self.frames()
        finally:
            self.event("KeyboardInput", {**value, "state": "Released"})
        self.frames()

    def capture(self, path):
        ticket = self.rpc("router/capture")["ticket"]
        deadline = time.monotonic() + 15
        while time.monotonic() < deadline:
            result = self.rpc("router/capture_status", {"ticket": ticket})
            if result["status"] == "complete":
                png = bytes(result["png"])
                if not png.startswith(b"\x89PNG\r\n\x1a\n"):
                    raise RuntimeError("Capture is not a PNG")
                pathlib.Path(path).write_bytes(png)
                return {"path": str(pathlib.Path(path).resolve()), "bytes": len(png)}
            time.sleep(0.05)
        raise TimeoutError("Screenshot did not complete within 15 seconds")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--url", default="http://127.0.0.1:15703")
    commands = parser.add_subparsers(dest="command", required=True)
    commands.add_parser("snapshot")
    find = commands.add_parser("find")
    find.add_argument("query", help="Find live nodes by name or displayed text")
    capture = commands.add_parser("capture")
    capture.add_argument("path")
    click = commands.add_parser("click")
    click.add_argument("x", type=float, help="Logical window coordinates")
    click.add_argument("y", type=float)
    key = commands.add_parser("key")
    key.add_argument("code", help="Bevy physical KeyCode, e.g. Space, Enter, KeyA")
    key.add_argument("--logical", help='JSON Key value, e.g. {"Character":"a"}')
    raw = commands.add_parser("rpc")
    raw.add_argument("method")
    raw.add_argument("params", nargs="?", default="null", help="JSON params")
    args = parser.parse_args()
    bridge = Bridge(args.url)
    result = None
    if args.command == "snapshot":
        result = bridge.snapshot()
    elif args.command == "find":
        result = [node for node in bridge.snapshot()["nodes"]
                  if args.query.casefold() in (node["name"] or "").casefold()
                  or args.query.casefold() in (node["text"] or "").casefold()]
    elif args.command == "capture":
        result = bridge.capture(args.path)
    elif args.command == "click":
        bridge.click(args.x, args.y)
    elif args.command == "key":
        logical = args.code
        if args.code.startswith("Key") and len(args.code) == 4:
            logical = {"Character": args.code[-1].lower()}
        elif args.code.startswith("Digit") and len(args.code) == 6:
            logical = {"Character": args.code[-1]}
        bridge.key(args.code, json.loads(args.logical) if args.logical else logical)
    elif args.command == "rpc":
        result = bridge.rpc(args.method, json.loads(args.params))
    print(json.dumps(result if result is not None else {"ok": True}, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    try:
        main()
    except (RuntimeError, OSError, ValueError) as error:
        print(f"UI debug: {error}", file=sys.stderr)
        sys.exit(1)
