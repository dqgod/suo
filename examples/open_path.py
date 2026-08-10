#!/usr/bin/env python3
"""Return a platform shell command that opens one trusted local path."""

from __future__ import annotations

import os
import shlex
import sys


def powershell_literal(value: str) -> str:
    return "'" + value.replace("'", "''") + "'"


def main(arguments: list[str]) -> int:
    if len(arguments) != 1:
        print("usage: open_path.py <file-or-directory>", file=sys.stderr)
        return 2

    path = os.path.abspath(os.path.expanduser(arguments[0]))
    if not os.path.exists(path):
        print(f"path does not exist: {path}", file=sys.stderr)
        return 2

    if sys.platform == "win32":
        print(f"Invoke-Item -LiteralPath {powershell_literal(path)}")
    else:
        print(f"/usr/bin/open {shlex.quote(path)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
