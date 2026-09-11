#!/usr/bin/env python3
"""Suo 脚本命令模板：接收 argv，并演示文字、图片和二维码结果协议。

一次执行只能选择一种结果类型：

- SUO_RESULT:text:    后面是要展示的文字，可输出多行。
- SUO_RESULT:image:   后面是 PNG/JPEG/WebP data URL，或纯 PNG Base64。
- SUO_RESULT:qrcode:  后面是原始文字/网址，由 Suo 在本地生成二维码。

普通 ``print()`` 可以记录 stdout 日志；只要出现任一结果前缀，Suo 就会
忽略未带前缀的 stdout 行。失败信息写入 stderr 并返回非 0 退出码。
"""

from __future__ import annotations

import base64
import sys


TEXT_RESULT_PREFIX = "SUO_RESULT:text:"
IMAGE_RESULT_PREFIX = "SUO_RESULT:image:"
QRCODE_RESULT_PREFIX = "SUO_RESULT:qrcode:"
SUPPORTED_IMAGE_MIME_TYPES = {"image/png", "image/jpeg", "image/webp"}


def emit_text_result(value: str) -> None:
    """返回文字；多行文字会为每一行加上 text 前缀。"""
    for line in value.splitlines() or [""]:
        print(f"{TEXT_RESULT_PREFIX} {line}")


def emit_image_result(image_bytes: bytes, mime_type: str = "image/png") -> None:
    """返回已经生成好的 PNG/JPEG/WebP 图片。

    示例：``emit_image_result(png_bytes, "image/png")``。
    Suo 会再次校验格式、尺寸和解码大小；不要传远程 URL 或 SVG。
    """
    if mime_type not in SUPPORTED_IMAGE_MIME_TYPES:
        raise ValueError("image must be PNG, JPEG, or WebP")
    encoded = base64.b64encode(image_bytes).decode("ascii")
    print(f"{IMAGE_RESULT_PREFIX} data:{mime_type};base64,{encoded}")


def emit_qrcode_result(value: str) -> None:
    """返回二维码原始内容；Suo 负责生成 PNG，脚本不需要图片依赖。"""
    value = value.strip()
    if not value:
        raise ValueError("QR code content must not be empty")
    if len(value.encode("utf-8")) > 500:
        raise ValueError("QR code content must not exceed 500 UTF-8 bytes")
    print(f"{QRCODE_RESULT_PREFIX} {value}")


def build_result(arguments: list[str]) -> str:
    """把 Suo 传入的参数转换为要展示的结果。

    输入 `demo alpha "two words"` 时，Suo 会先移除关键字 `demo`，再按
    引号感知规则把剩余文本安全拆成 `["alpha", "two words"]`。Suo 不会
    替脚本展开 `~`、`*`、环境变量、管道或重定向；需要这些语义时，应在
    这里用对应语言的安全 API 明确实现。

    参数数量、顺序、可选性和含义都由这个函数决定。零参数脚本也是合法的。
    """
    if not arguments:
        return "Hello from the Suo script template"

    # TODO: 在这里完成自己的业务逻辑。示例只把每个独立参数清楚地展示出来。
    return " | ".join(f"arg{index}={value}" for index, value in enumerate(arguments))


def main(arguments: list[str]) -> int:
    """默认返回文字；按业务需要改为 image 或 qrcode，但不要同时输出。

    - 文字：``emit_text_result("第一行\n第二行")``。
    - 图片：``emit_image_result(png_bytes, "image/png")``。
    - 二维码：``emit_qrcode_result("https://www.example.com")``。
    - 成功时返回退出码 0；失败时把可读错误写到 stderr 并返回非 0。
    - 普通 stdout 日志可以使用 print()；出现结果前缀后，未标记日志会忽略。
      没有任何前缀的旧脚本仍返回完整 stdout。
    - 不要把密钥或其他敏感信息写入 stdout 或 stderr。
    - “复制返回文本”或“执行返回的 Shell 命令”由设置页的返回值动作决定，
      不是脚本自行决定。默认应选择更安全的“复制返回文本”。
    """
    try:
        result = build_result(arguments)
        if not result:
            raise ValueError("result must not be empty")
    except (OSError, ValueError) as error:
        print(f"script failed: {error}", file=sys.stderr)
        return 2

    print("script completed")  # 示例日志：不会进入最终搜索结果。
    emit_text_result(result)
    return 0


if __name__ == "__main__":
    # Suo 已把关键字去掉，所以这里只读取真正的脚本参数。
    raise SystemExit(main(sys.argv[1:]))
