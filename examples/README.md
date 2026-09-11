# Suo 脚本命令示例

从 [`script_template.py`](script_template.py) 开始最直接。它是 Windows 与 macOS 共用的 Python 模板，完整展示了 Suo 当前脚本接口：接收独立 argv、用退出码报告成功或失败，并以类型前缀明确选择最终结果。

## 在设置中添加模板

进入“设置 → 命令与服务 → 脚本命令”，新增一项并填写：

| 设置项 | 建议值 | 含义 |
| --- | --- | --- |
| 名称 | `脚本模板` | 只用于结果展示 |
| 关键字 | `demo` | 启动器中输入的触发词；不传给脚本 |
| 运行时 | `Python` | Suo 会依次寻找 `python`、`python3` |
| 本地脚本路径 | `scripts/script_template.py` | 首次启动自动初始化；也可填写绝对路径或 `~/...` |
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
2. 成功时返回退出码 `0`，并优先用区分大小写的 `SUO_RESULT:text:`、`SUO_RESULT:image:` 或 `SUO_RESULT:qrcode:` 标记结果。旧 `SUO_RESULT:` 继续等同文本；只要出现至少一个结果标记，其他 stdout 日志就不会进入结果；完全没有前缀的旧脚本仍把完整 stdout 作为文本结果。
3. 失败时返回非 `0`，把给用户看的原因写入 stderr；Suo 会显示 stderr，而不是成功结果。
4. stdout 与 stderr 合计不能超过 1 MB，运行时间不能超过该命令配置的超时；查询变化或关闭时，Suo 会取消进程树。
5. 脚本工作目录是脚本所在目录。`scripts/...` 相对路径优先从平台默认应用配置目录的用户脚本文件夹解析；其他相对路径随后兼容应用 bundle 资源与开发源码目录。自定义脚本也可使用绝对路径或 `~/...`。
6. 当前是单结果协议：文本可多行，但一次执行不能同时返回文本和图片，也只能返回一张图片。`suo-json-v1` 多结果协议仍是后续能力，现阶段不要让模板输出该 JSON。
7. Python 脚本的 stdout/stderr 会由 Suo 指定为 UTF-8；其他程序先按 UTF-8 解码，Windows 下失败时回退到当前系统代码页。首尾空白仍会去掉，中间的换行会保留，启动器最多展示前两行，例如 `print(f"第一行\\n第二行")`。

## 明确选择返回内容

模板中的 `emit_text_result()` 会给结果的每一行自动添加 `SUO_RESULT:text:`：

```python
print("正在加载缓存")                 # 普通日志，不进入最终结果
emit_text_result("第一行\n第二行")   # 两行都会返回给搜索框
print("执行结束")                     # 普通日志，不进入最终结果
```

对应原始 stdout 是：

```text
正在加载缓存
SUO_RESULT:text: 第一行
SUO_RESULT:text: 第二行
执行结束
```

Suo 最终只返回 `第一行\n第二行`。此前缀仅处理成功脚本的 stdout；错误退出时 stderr 仍完整显示，脚本结果触发的后续 Shell 输出也保持普通文本语义。stdout 与 stderr 仍共同受 1 MB 上限约束，因此不要输出无界日志。

## 类型化图片与二维码

- `SUO_RESULT:image: data:image/png;base64,...` 返回一张 PNG；JPEG、WebP 也可使用各自完整 data URL。仅给 Base64 内容时按 PNG 解释。
- `SUO_RESULT:qrcode: 任意文字或网址` 让 Suo 在本地生成二维码 PNG；参考 [`qr.py`](qr.py)。内容不超过 500 个 UTF-8 字节，中文与 emoji 会带 UTF-8 字符集声明；它不需要安装 Python 图片依赖，也不访问第三方二维码服务。
- 返回图片只允许 PNG/JPEG/WebP，解码后不超过 512 KB、最长边不超过 1024 px；远程 URL、SVG、损坏图片、多张图片及文本/图片混合输出都会被拒绝。
- “复制返回文本”模式下，普通图片按 Enter 复制其 data URL；二维码按 Enter 复制生成二维码前的原始内容。“执行返回的 Shell 命令”不接受任何图片结果。

[`script_template.py`](script_template.py) 已提供三个可直接复用的函数：

```python
from pathlib import Path

emit_text_result("第一行\n第二行")
emit_image_result(Path("result.png").read_bytes(), "image/png")
emit_qrcode_result("https://www.example.com")
```

上面三行是三种互斥用法示例，单次执行只能调用其中一种。`emit_image_result()` 适合脚本已经生成好图片的情况；`emit_qrcode_result()` 传入的只是原始内容，二维码图片由 Suo 生成。

默认配置中的二维码命令为：关键词 `qr`、脚本路径 `scripts/qr.py`、即时执行延迟 `150 ms`。例如：

```text
qr www.google.com
qr "包含空格的一段文字"
```

## 两种返回值动作

“复制返回文本”是默认安全行为。按 Enter 执行模式下，第一次 Enter 运行脚本并显示 stdout，第二次 Enter 或点击结果才复制它；即时执行模式会自动产生结果，之后按一次 Enter 即复制。

“执行返回的 Shell 命令”是显式高风险行为：stdout 必须是一条完整命令，第二次激活结果后，macOS 才通过 `/bin/bash -lc`、Windows 才通过非交互 PowerShell 执行它。不要把未校验的用户参数直接拼入返回命令。参考 [`open_path.py`](open_path.py)：它先验证本地路径，再分别生成经过引用的 macOS / Windows 打开命令。

现有 [`timestamp.py`](timestamp.py) 则是“即时执行 + 复制返回文本”的完整示例，可参考它的参数校验、stderr 和退出码处理。
