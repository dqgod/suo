#!/usr/bin/env python3
"""Convert a Unix millisecond timestamp using local time or an explicit offset."""

from __future__ import annotations

from datetime import datetime, timedelta, timezone
import re
import sys
from typing import Optional


OFFSET_PATTERN = re.compile(r"^([+-])(\d{1,2})(?::?(\d{2}))?$")


def parse_timezone(value: str) -> timezone:
    match = OFFSET_PATTERN.fullmatch(value)
    if not match:
        raise ValueError("timezone must look like +8, -5, +08:30, or -0530")
    sign, hours_text, minutes_text = match.groups()
    hours = int(hours_text)
    minutes = int(minutes_text or "0")
    if hours > 23 or minutes > 59:
        raise ValueError("timezone offset is out of range")
    offset = timedelta(hours=hours, minutes=minutes)
    if sign == "-":
        offset = -offset
    return timezone(offset)


def convert(value: str, target_timezone: Optional[timezone] = None) -> str:
    timestamp_ms = int(value)
    converted = datetime.fromtimestamp(timestamp_ms / 1000, tz=target_timezone)
    return converted.strftime("%Y-%m-%d %H:%M:%S")


def main(arguments: list[str]) -> int:
    if not 1 <= len(arguments) <= 2:
        print("usage: timestamp.py <milliseconds> [timezone offset]", file=sys.stderr)
        return 2

    try:
        target_timezone = parse_timezone(arguments[1]) if len(arguments) == 2 else None
        print(convert(arguments[0], target_timezone))
    except (ValueError, OSError, OverflowError) as error:
        print(f"invalid timestamp: {error}", file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
