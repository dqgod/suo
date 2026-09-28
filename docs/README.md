# Suo 文档与 UI 方案

本目录保存当前产品需求、正在评审的 UI 方案，以及已经过期但仍可追溯的历史稿。

## 当前文档

- [PRODUCT_REQUIREMENTS.md](./PRODUCT_REQUIREMENTS.md)：需求方案、技术架构、数据结构与验收标准。
- [ROADMAP.md](./ROADMAP.md)：`dev` 当前状态、当前发布与平台验证状态，以及按优先级维护的 TODO / 迭代路线图。
- [ui-settings-redesign-proposal.html](./ui-settings-redesign-proposal.html)：设置中心整体改版评审稿，包含 A 浅色原生、B 分栏工具、C 深色改良三个方向；支持通用、搜索、脚本、网页搜索、翻译、外观页面，以及命令编辑、离开保护和超出滑块常用范围的字号输入。配套 CSS / JS 需保留在同目录。
- [ui-settings-redesign-review.md](./ui-settings-redesign-review.md)：当前代码中的交互问题、数值限制来源、设计取舍和验证范围。B 方向已进入正式界面实现，文件保留改版前证据与设计取舍；验证与 Mac 交接见 `handoff/`。
- [ui-appearance-management-proposal.html](./ui-appearance-management-proposal.html)：第四版外观设计交互稿。皮肤作用域、运行中皮肤、正在编辑的皮肤和设置保存策略分别表达；顶部下拉框是唯一的运行皮肤切换入口，皮肤卡片只选择编辑对象。
- [ui-appearance-management-launcher.png](./ui-appearance-management-launcher.png)：第四版搜索界面皮肤编辑状态，展示“统一保存设置”开启、运行中皮肤与编辑对象不同，以及应用/文件夹/文件图标验收样例。
- [ui-appearance-management-settings.png](./ui-appearance-management-settings.png)：第四版设置界面皮肤编辑状态，展示“统一保存设置”关闭后的即时写入模式。

HTML 原型不需要安装依赖，可以直接在浏览器中打开。第四版外观稿支持点击切换范围、编辑对象、运行皮肤和保存策略，也可通过 `?view=launcher-manual` 或 `?view=settings-instant` 直接打开两张预览对应的状态。整体改版稿使用 `?direction=native|studio|refined&page=general|search|scripts|web|translation|appearance` 指定初始画面。两份稿都只用于交互和视觉评审，不会扫描文件、执行脚本、调用翻译服务或保存正式配置。

## 过期 UI 稿

- [archive/ui-v1](./archive/ui-v1/)：第一版综合原型。
- [archive/ui-v2](./archive/ui-v2/)：第二版折叠设置与“搜索/设置共用皮肤”原型。
- [archive/ui-v3](./archive/ui-v3/)：第三版搜索/设置皮肤分离原型；运行中皮肤与编辑对象尚未分成两套状态。

归档内容不代表当前实现，不应作为新功能的开发依据；保留它们仅用于追溯产品演进。
