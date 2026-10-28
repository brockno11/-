# 轻笺 Skills 与质量审查工作流

## 目的

项目只保留四个互补的 skills：一个需求压力测试、一个现有产品 UX 审计、一个 UI/UX 设计资料库，以及一个轻笺专用更新守门人。Codex 从仓库根目录的 `.agents/skills` 自动发现它们；`AGENTS.md` 决定何时介入。

所有审查遵循“先报告、后批准”。审查可以写入 `.reviews/`，但不能修改业务代码；修复必须由用户针对具体 findings 另行批准。

## 已安装 Skills

| 调用 | 用途 | 固定来源 |
| --- | --- | --- |
| `$grill-me` | 对产品、功能和交互方案做逐轮决策压力测试 | `mattpocock/skills` commit `6654f6b60cd9d5be8b54c6fafe44346dabeb3b76`，MIT；使用实际 `grilling` 引擎并改名适配 Codex |
| `$ux-audit` | 对已存在界面做基于证据的 UX、无障碍和设计系统审计 | `paulunemoon/ux-audit-skill` commit `f07ff7607039a846fba1e711bb99716a68b12f34`，MIT；移除不兼容的顶层 `version` 字段 |
| `$ui-ux-pro-max` | 查询 Svelte 5、桌面 UI、布局、交互、可访问性和设计系统资料 | npm `ui-ux-pro-max-cli@2.15.0`；integrity `sha512-D0J/C40xrzzi5si6ZLtRGbEE5v3QjL7d4wJNnasmP3yfDSrGiuqVCdwQiqCNnIkbqOuVoA/uonR2o1WKXh3urw==`，MIT |
| `$qingjian-update-guardian` | 结合轻笺约束、机械测试和独立第三视角做分级更新审查 | 项目自有 skill |

第三方来源：

- <https://github.com/mattpocock/skills>
- <https://github.com/paulunemoon/ux-audit-skill>
- <https://github.com/nextlevelbuilder/ui-ux-pro-max-skill>

## 调用方式

```text
$grill-me 下一阶段怎样提升轻笺的每日使用价值？
$ui-ux-pro-max 为 Windows 本地优先的 Svelte 5 + Tauri 便签提供克制高级的设计约束
$ux-audit quick 审查当前工作台和设置流程
$ux-audit tokens 检查颜色、间距、圆角和组件漂移
$qingjian-update-guardian quick
$qingjian-update-guardian ui
$qingjian-update-guardian release
```

日常改动使用最小充分模式。UI 模式在代码证据之外尽量查看真实渲染；发布模式要求产品体验和工程/数据安全两个独立审查视角。完整协议位于 `.agents/skills/qingjian-update-guardian/references/review-protocol.md`。

## 报告与发布

- 日常审查写入 `.reviews/<时间>-<模式>/`，该目录被 Git 忽略。
- 正式发布经过修复、复测和用户接受风险后，将精简摘要归档到 `docs/reviews/releases/v<版本>.md`。
- 摘要必须列出实际执行的命令、退出码、跳过项、已解决阻断项、剩余接受风险与最终结论。
- `qa:windows` 依赖真实的 `release/current/qingjian.exe`。文件不存在或不是本次待发布构建时必须写 `SKIPPED`，不能按通过处理。

## 确定性质量门

- `npm run quality:static`：Svelte/TypeScript 检查、Vitest、Vite 构建。
- `npm run quality:release`：版本一致性、静态质量门、Rust 测试。
- GitHub 标签发布工作流执行 `quality:release` 后才构建、签名和发布安装包。
- Release workflow 的第三方 Actions 固定到审核过的完整 commit SHA；`workflow_dispatch` 只允许从 `main` 运行，并在 Release 正文记录 source commit。
- Windows QA 在启动前强制比对 EXE 的 FileVersion、ProductVersion 与 `package.json`，旧版本产物会直接失败。

## 升级第三方 Skills

禁止在应用版本更新时自动升级 skills，也不要在项目内使用无版本的 `@latest`。

1. 在仓库之外的临时目录下载候选固定版本。
2. 对比 `SKILL.md`、references、scripts、许可证和生成文件清单。
3. 搜索网络访问、子进程、动态执行、文件删除和外部写入；任何新权限都要单独评估。
4. 使用 `skill-creator/scripts/quick_validate.py` 验证目录，并运行 skill 自带的离线脚本或测试。
5. 用轻笺的真实请求做冒烟测试，确认触发边界、输出和只读约束。
6. 评审通过后替换项目副本，同时更新本文件的提交、版本和 integrity。

如果 Codex 当前会话没有显示刚安装或更新的 skill，重新打开项目任务，使技能清单和 `AGENTS.md` 指令链重新加载。
