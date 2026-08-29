# Suo 脚本命令示例

从 [`script_template.py`](script_template.py) 开始最直接。它是 Windows 与 macOS 共用的 Python 模板，完整展示了 Suo 当前脚本接口：接收独立 argv、用退出码报告成功或失败、把一个纯文本结果写到 stdout。

## 在设置中添加模板

进入“设置 → 命令与服务 → 脚本命令”，新增一项并填写：

| 设置项 | 建议值 | 含义 |
| --- | --- | --- |
| 名称 | `脚本模板` | 只用于结果展示 |
| 关键字 | `demo` | 启动器中输入的触发词；不传给脚本 |
| 运行时 | `Python` | Suo 会依次寻找 `python`、`python3` |
| 本地脚本路径 | `examples/script_template.py` | 也可填写绝对路径或 `~/...` |
| 返回值动作 | `复制返回文本（默认）` | 第二次激活结果时复制 stdout |
| 执行方式 | `按 Enter 执行` | 先明确触发脚本，适合起步和有副作用的任务 |
| 超时 | `3000` ms | 可按任务调整，范围 100–60000 ms |

保存后可输入：

```text
demo
demo alpha beta
demo alpha "two words"
```

最后一条命令实际传给 Python 的参数等价于：

```text
python script_template.py alpha "two words"
```

脚本收到的 `sys.argv[1:]` 是 `["alpha", "two words"]`。Suo 不通过 Shell 拼接输入，因此 `*`、`$HOME`、`%USERPROFILE%`、`|`、`>` 等内容默认都是普通参数，不会被二次执行或展开。

## 脚本与 Suo 的约定

1. 参数 Schema 由脚本所有。脚本应自行校验参数数量、格式和含义；Suo 只做引号感知的 argv 拆分。
2. 成功时返回退出码 `0`，并把要展示的最终纯文本写入 stdout。当前版本会把完整 stdout 作为一个结果，去掉首尾空白。
3. 失败时返回非 `0`，把给用户看的原因写入 stderr；Suo 会显示 stderr，而不是成功结果。
4. stdout 与 stderr 合计不能超过 1 MB，运行时间不能超过该命令配置的超时；查询变化或关闭时，Suo 会取消进程树。
5. 脚本工作目录是脚本所在目录。相对脚本路径依次在应用 bundle 资源、配置目录和开发源码目录中解析；自定义脚本建议使用绝对路径或 `~/...`。
6. 当前可用输出是纯文本单结果。`suo-json-v1` 多结果协议仍是后续能力，现阶段不要让模板输出该 JSON。

## 两种返回值动作

“复制返回文本”是默认安全行为。按 Enter 执行模式下，第一次 Enter 运行脚本并显示 stdout，第二次 Enter 或点击结果才复制它；即时执行模式会自动产生结果，之后按一次 Enter 即复制。

“执行返回的 Shell 命令”是显式高风险行为：stdout 必须是一条完整命令，第二次激活结果后，macOS 才通过 `/bin/bash -lc`、Windows 才通过非交互 PowerShell 执行它。不要把未校验的用户参数直接拼入返回命令。参考 [`open_path.py`](open_path.py)：它先验证本地路径，再分别生成经过引用的 macOS / Windows 打开命令。

现有 [`timestamp.py`](timestamp.py) 则是“即时执行 + 复制返回文本”的完整示例，可参考它的参数校验、stderr 和退出码处理。
