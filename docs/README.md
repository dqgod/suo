# Suo 文档与 UI 方案

本目录保存当前产品需求、正在评审的 UI 方案，以及已经过期但仍可追溯的历史稿。

## 当前文档

- [PRODUCT_REQUIREMENTS.md](./PRODUCT_REQUIREMENTS.md)：需求方案、技术架构、数据结构与验收标准。
- [ui-appearance-split-proposal.html](./ui-appearance-split-proposal.html)：第三版外观设计交互稿。搜索界面与设置界面分别维护内置皮肤、自定义皮肤和实时预览；搜索皮肤还能独立控制窗口、搜索框、结果行、选中态、文字和图标显隐。
- [ui-appearance-split-launcher.png](./ui-appearance-split-launcher.png)：第三版搜索界面皮肤编辑器静态预览图。
- [ui-appearance-split-settings.png](./ui-appearance-split-settings.png)：第三版设置界面皮肤编辑器静态预览图。

HTML 原型不需要安装依赖，可以直接在浏览器中打开。当前稿支持通过 `?scope=launcher` 和 `?scope=settings` 直接打开两种皮肤编辑器。它只用于交互和视觉评审，不会扫描文件、执行脚本、调用翻译服务或保存正式配置。

## 过期 UI 稿

- [archive/ui-v1](./archive/ui-v1/)：第一版综合原型。
- [archive/ui-v2](./archive/ui-v2/)：第二版折叠设置与“搜索/设置共用皮肤”原型。

归档内容不代表当前实现，不应作为新功能的开发依据；保留它们仅用于追溯产品演进。
