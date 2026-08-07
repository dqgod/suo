# Suo Launcher 产品方案

Suo 是一个面向 macOS 与 Windows 的轻量快捷启动器概念，核心体验是“按下快捷键、输入、回车完成”。

本目录包含：

- [PRODUCT_REQUIREMENTS.md](./PRODUCT_REQUIREMENTS.md)：中文需求方案、技术架构、数据结构、里程碑与验收标准。
- [ui-prototype.html](./ui-prototype.html)：无需安装依赖的交互式 UI 原型，浏览器直接打开即可体验。
- [ui-preview.png](./ui-preview.png)：启动器主界面的静态预览图。
- [ui-preview-google.png](./ui-preview-google.png)：`google codex` 网络搜索命令预览。
- [ui-preview-commands.png](./ui-preview-commands.png)：命令与脚本设置页预览。
- [ui-preview-appearance.png](./ui-preview-appearance.png)：主题编辑页预览。

## 查看 UI 原型

在 PowerShell 中执行：

```powershell
Start-Process .\ui-prototype.html
```

原型内置以下演示：

- 输入 `11+1` 查看计算结果；
- 输入 `fy hello` 查看翻译结果；
- 输入 `ts 1786082576069` 查看脚本命令结果；
- 输入 `google codex`，按 `Enter` 在浏览器打开对应的 Google 搜索；
- 输入普通关键词体验应用与文件混合搜索；
- 使用方向键切换结果、`Enter` 执行动作、`Esc` 清空或关闭；
- 打开“命令设置”和“主题外观”查看管理界面并实时换肤。

也可以通过查询参数直接打开指定页面：`ui-prototype.html?view=commands`、`ui-prototype.html?view=appearance`，或用 `ui-prototype.html?q=google%20codex` 预填演示查询。

> 说明：HTML 是交互与视觉原型，不会真的扫描本机文件、调用翻译服务或执行脚本；网络搜索演示会在用户按 `Enter` 后打开普通浏览器标签页。相关真实行为与安全约束已在需求方案中定义。
