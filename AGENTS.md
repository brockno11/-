# 轻笺项目约定

## 不可退让的产品约束

- 轻笺是 Windows 10/11 纯本地便签。未经用户明确批准，不得引入遥测、账号、云同步或新的联网行为。
- 修改存储、迁移、保存、退出、更新器、窗口恢复或备份逻辑时，优先保护数据兼容与可恢复性。
- 保持克制、清晰的中性白灰视觉方向；设计资料是参考，不得擅自套用新的模板化风格。
- 审查默认只报告。未经用户针对具体 findings 的批准，不得自动修复、提交、打标签或发布。

## Skills 触发矩阵

- 产品构想、新功能、流程或重要交互决策：实现前使用 `$grill-me` 澄清决策树。
- Svelte、CSS、布局、文案、编辑器、焦点、键盘或窗口交互：实现前使用 `$ui-ux-pro-max` 作为设计参考，渲染后使用 `$ux-audit quick`；设计系统漂移使用 `$ux-audit tokens`。
- 普通代码更新完成后：使用 `$qingjian-update-guardian quick`。
- UI 或交互更新完成后：使用 `$qingjian-update-guardian ui`。
- 版本、更新器、签名、安装包、存储迁移、启动/退出、托盘或正式发布：使用 `$qingjian-update-guardian release`。

## 验证要求

- TypeScript、Svelte、共享 UI 或构建配置变化：运行 `npm run quality:static`。
- 正式发布或发布关键路径变化：运行 `npm run quality:release`。
- 只有 `release/current/qingjian.exe` 是本次待验证正式构建时才运行 `npm run qa:windows`；否则明确记录 `SKIPPED` 和缺失前置条件。
- 详细流程、版本锁定与报告策略见 `docs/QUALITY_WORKFLOW.md`。
