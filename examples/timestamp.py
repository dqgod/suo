#!/usr/bin/env python3
"""Convert one or more Unix millisecond timestamps to local time."""

from __future__ import annotations

from datetime import datetime
import sys


def convert(value: str) -> str:
    timestamp_ms = int(value)
    return datetime.fromtimestamp(timestamp_ms / 1000).strftime("%Y-%m-%d %H:%M:%S")


def main(arguments: list[str]) -> int:
    if not arguments:
        print("usage: timestamp.py <milliseconds> [more milliseconds]", file=sys.stderr)
        return 2

    try:
        for argument in arguments:
            print(convert(argument))
    except (ValueError, OSError, OverflowError) as error:
        print(f"invalid timestamp: {error}", file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
