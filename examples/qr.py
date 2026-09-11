#!/usr/bin/env python3
"""Suo 二维码示例：把 argv 合并成二维码内容，由 Suo 本地渲染为 PNG。"""

from __future__ import annotations

import sys


RESULT_PREFIX = "SUO_RESULT:qrcode:"
MAX_CONTENT_BYTES = 500


def main(arguments: list[str]) -> int:
    content = " ".join(arguments).strip()
    if not content:
        print("usage: qr <text or URL>", file=sys.stderr)
        return 2
    if len(content.encode("utf-8")) > MAX_CONTENT_BYTES:
        print(f"QR content exceeds {MAX_CONTENT_BYTES} UTF-8 bytes", file=sys.stderr)
        return 2

    # qrcode is a typed Suo result. The script needs no Python image package:
    # Suo creates and validates the local PNG before it reaches the webview.
    print(f"{RESULT_PREFIX} {content}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
