# 轻笺

轻笺是一款面向 Windows 10/11 的纯本地富文本便签。当前版本为 **0.7.0**，包含便签工作台、多张独立桌面便签、多选管理、归档与回收站、文件夹和标签、全文搜索，以及轻量窗口阴影。

## 正式下载与更新

- [下载最新 Windows 正式版](https://github.com/brockno11/-/releases/latest)
- [查看全部历史版本](https://github.com/brockno11/-/releases)
- [查看详细更新记录](CHANGELOG.md)

正式版 Release 提供 Windows x64 安装包、更新签名和版本清单。轻笺支持在“设置 → 软件更新”中手动检查，也可在启动后自动检查；发现新版本时会展示更新说明，用户确认后才下载、安装并重启。

仓库中的 [`release/0.4.3`](release/0.4.3) 仅作为旧版离线构建存档，之后统一以 GitHub Releases 作为正式下载来源。

数据默认保存在 `%LOCALAPPDATA%\com.qingjian.desktop`。启动、编辑、搜索、保存、导出和恢复均为本地能力；只有用户主动检查或启用启动检查更新时才会访问 GitHub。轻笺不会联网同步，也不会收集遥测数据。

## 主要功能

- 便签新建、搜索、置顶、归档、回收站与批量操作
- 每张便签拥有独立窗口，可在多行胶囊、卡片、编辑三种形态之间切换
- 独立置顶、背景/文字双层透明度、完全鼠标穿透锁定与可关闭的四边贴边折叠
- 正文首个非空行自动成为标题，卡片无需维护单独标题栏
- 多显示器、DPI 与每种形态的位置和大小恢复；开机只恢复桌面便签，不弹工作台
- 文件夹、标签和最近编辑视图
- 富文本编辑、字体字号与行距、卡片右键格式菜单、任务列表、表格和图片
- Markdown 快捷输入：标题、列表、待办、引用、代码与分隔线会即时转为整洁排版
- 圆圈待办：点击空心圆完成事项，完成后显示对号并弱化文字
- 多种复制方式，包括纯文本、HTML 和源格式
- 一键复制或导出 Markdown；导出时会自动整理随文图片资源
- 串行修订自动保存、失败重试、退出前落盘、全局快捷键、托盘运行与开机启动
- 启动数据库健康检查、迁移前备份与最近七份每日安全快照
- 中性白灰默认主题，以及按类别即时保存、失败自动回滚的设置页
- GitHub 签名更新、手动/启动检查、更新提醒与一键安装重启
- 透明窗口边缘、1px 细边框和轻柔黑色阴影

## 快捷键

- `Ctrl+Alt+N`：直接打开上次使用的便签；没有可用便签时自动新建
- `Ctrl+Alt+L`：解锁全部鼠标穿透便签
- `Ctrl+Alt+O`：恢复全部便签可见度（不显示隐藏便签）
- `Ctrl+Alt+H`：显示 / 隐藏全部桌面便签
- `Ctrl+N`：新建便签
- `Ctrl+S`：立即保存
- `Ctrl+W`：保存并隐藏当前桌面便签
- `Ctrl+,`：打开设置
- `Esc`：编辑形态保存并回到卡片形态

## 本地开发

需要 Node.js、Rust stable MSVC 和 Visual Studio 2022 C++ Build Tools。

```powershell
npm install
npm run check
npm test
npm run qa:windows
npm run tauri:dev
```

`qa:windows` 会针对 `release/current/qingjian.exe` 调用真实的“隐藏到系统托盘”按钮，执行 50 轮关闭与桌面入口唤回，并检查单实例、最小化状态、响应状态和生产包中的开发服务器地址。请在生成最新正式包后运行。

`tauri:dev` 才会连接本机 Vite 开发服务器。不要把 `src-tauri/target/debug/qingjian.exe`
当作正式入口或为它创建桌面快捷方式；日常使用请启动安装版或 `target/release` 中的独立版本。

## 构建 Windows 版本

```powershell
.\scripts\build-windows.ps1
```

构建产物默认生成在 `src-tauri\target\release`，发布前应将最终文件复制到 `release/<版本号>`。

正式发布、更新签名和版本命名规则见 [`docs/RELEASING.md`](docs/RELEASING.md)。
