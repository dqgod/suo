#!/usr/bin/env python3
"""Convert timestamps and local date/time values with an optional UTC offset.

Usage:
  ts
  ts now [offset]
  ts <10-digit seconds | 13-digit milliseconds> [offset]
  ts <YYYY-MM-DD> <HH:MM:SS> [offset]
  ts help

Offsets default to UTC+8 and accept forms such as 8, -5, +08:30, or -0530.
"""

from __future__ import annotations

from datetime import datetime, timedelta, timezone
import re
import sys


OFFSET_PATTERN = re.compile(r"^([+-])?(\d{1,2})(?::?(\d{2}))?$")
TIMESTAMP_PATTERN = re.compile(r"^(?:\d{10}|\d{13})$")
RESULT_PREFIX = "SUO_RESULT:text:"
DEFAULT_TIMEZONE = "8"


def emit_result(value: str) -> None:
    """Emit only the marked text lines that Suo should display."""
    for line in value.splitlines() or [""]:
        print(f"{RESULT_PREFIX} {line}")


def parse_timezone(value: str) -> timezone:
    match = OFFSET_PATTERN.fullmatch(value)
    if not match:
        raise ValueError("timezone must look like 8, -5, +08:30, or -0530")

    sign, hours_text, minutes_text = match.groups()
    hours = int(hours_text)
    minutes = int(minutes_text or "0")
    if hours > 23 or minutes > 59:
        raise ValueError("timezone offset is out of range")

    offset = timedelta(hours=hours, minutes=minutes)
    if sign == "-":
        offset = -offset
    return timezone(offset)


def format_timezone_label(value: timezone) -> str:
    offset = value.utcoffset(None)
    if offset is None:
        raise ValueError("timezone has no UTC offset")

    total_minutes = int(offset.total_seconds() / 60)
    sign = "+" if total_minutes >= 0 else "-"
    hours, minutes = divmod(abs(total_minutes), 60)
    if minutes:
        return f"UTC{sign}{hours:02d}:{minutes:02d}"
    return f"UTC{sign}{hours}"


def normalize_timestamp(value: str) -> float:
    """Return seconds for a 10-digit second or 13-digit millisecond value."""
    if not TIMESTAMP_PATTERN.fullmatch(value):
        raise ValueError("timestamp must be 10-digit seconds or 13-digit milliseconds")
    timestamp = int(value)
    return timestamp / 1000.0 if len(value) == 13 else float(timestamp)


def print_help() -> None:
    emit_result(
        "ts - Timestamp / datetime converter\n"
        "ts\n"
        "ts now [offset]\n"
        "ts <10-digit seconds | 13-digit milliseconds> [offset]\n"
        "ts <YYYY-MM-DD> <HH:MM:SS> [offset]\n"
        "offset: 8 / -5 / +08:30 / -0530 (default: UTC+8)"
    )


def usage_error(message: str) -> int:
    print(f"error: {message}", file=sys.stderr)
    print(
        "usage: timestamp.py [now [offset] | <timestamp> [offset] | "
        "<YYYY-MM-DD> <HH:MM:SS> [offset] | help]",
        file=sys.stderr,
    )
    return 2


def main(arguments: list[str]) -> int:
    try:
        if not arguments:
            target_timezone = parse_timezone(DEFAULT_TIMEZONE)
            now = datetime.now(target_timezone)
            timestamp_ms = int(now.timestamp() * 1000)
            emit_result(
                f"当前时间：{now.strftime('%Y-%m-%d %H:%M:%S')} "
                f"{format_timezone_label(target_timezone)}\n"
                f"Unix时间戳(毫秒)：{timestamp_ms}"
            )
            return 0

        if arguments[0] == "help":
            if len(arguments) != 1:
                return usage_error("help does not accept additional arguments")
            print_help()
            return 0

        if arguments[0] == "now":
            if len(arguments) > 2:
                return usage_error("now accepts at most one timezone offset")
            target_timezone = parse_timezone(
                arguments[1] if len(arguments) == 2 else DEFAULT_TIMEZONE
            )
            now = datetime.now(target_timezone)
            timestamp_ms = int(now.timestamp() * 1000)
            emit_result(
                f"当前时间：{now.strftime('%Y-%m-%d %H:%M:%S')} "
                f"{format_timezone_label(target_timezone)}\n"
                f"Unix时间戳(毫秒)：{timestamp_ms}"
            )
            return 0

        target_timezone = parse_timezone(DEFAULT_TIMEZONE)
        if len(arguments) in (1, 2) and TIMESTAMP_PATTERN.fullmatch(arguments[0]):
            if len(arguments) == 2:
                target_timezone = parse_timezone(arguments[1])
            converted = datetime.fromtimestamp(
                normalize_timestamp(arguments[0]), tz=target_timezone
            )
            emit_result(
                f"{converted.strftime('%Y-%m-%d %H:%M:%S')} "
                f"{format_timezone_label(target_timezone)}"
            )
            return 0

        if len(arguments) not in (2, 3):
            return usage_error("expected a timestamp or a date and time")

        if len(arguments) == 3:
            target_timezone = parse_timezone(arguments[2])
        local_value = datetime.strptime(
            f"{arguments[0]} {arguments[1]}", "%Y-%m-%d %H:%M:%S"
        ).replace(tzinfo=target_timezone)
        emit_result(str(int(local_value.timestamp() * 1000)))
        return 0
    except (ValueError, OSError, OverflowError) as error:
        return usage_error(str(error))


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
