# Windows v0.1.2 superseded handoff summary

本文件归档 `v0.1.2` Windows 资产与验证状态；完整逐项历史可通过本文件归档时的 Git 版本追溯。当前 Windows 工作只执行 [`../WINDOWS.md`](../WINDOWS.md) 的 `v0.1.3` 清单。

- 不可变 tag：`v0.1.2`，提交 `74c27fa9279a4807c5704c0188f75096576e06de`。
- 已发布 `Suo_0.1.2_x64-setup.exe`（4,023,263 bytes）与 `.sha256`；安装包 SHA-256 为 `88C4E797300B828821701483E8B83E72E99755895E910B51AA3E96FFEF0D0FB6`。
- 自动化构建、配置 v16/v17、类型化文字/图片/二维码结果、UTF-8/CP936 输出、用户脚本初始化、Windows 文件定位、删除二次确认和时间戳参数校验已有代码与测试证据。
- 当时未关闭的安装包界面、Finder/Explorer 类真实交互不再直接套用到旧安装包；`v0.1.3` 必须从新 tag 重新构建并执行当前清单，不能把 `v0.1.2` 资产改名复用。
