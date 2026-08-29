#!/usr/bin/env python3
"""Suo 脚本命令模板：接收 argv，并把一个纯文本结果写到 stdout。"""

from __future__ import annotations

import sys


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
    """遵循 Suo 当前的纯文本脚本协议。

    - 成功：把最终结果写到 stdout，并返回退出码 0。
    - 失败：把可读错误写到 stderr，并返回非 0 退出码。
    - 不要把调试日志、密钥或多段协议数据混入 stdout；stdout 的完整文本会
      作为一个结果展示。
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

    print(result)
    return 0


if __name__ == "__main__":
    # Suo 已把关键字去掉，所以这里只读取真正的脚本参数。
    raise SystemExit(main(sys.argv[1:]))
