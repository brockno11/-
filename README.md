# 轻笺

轻笺是一款面向 Windows 10/11 的纯本地富文本便签。当前版本为 **0.4.3**，包含便签工作台、独立编辑器、多选管理、归档与回收站、文件夹和标签、全文搜索，以及轻量黑色窗口阴影。

## 下载与运行

最新 Windows 构建位于 [`release/0.4.3`](release/0.4.3)：

- `轻笺_0.4.3_x64-setup.exe`：Windows x64 安装包
- `qingjian-0.4.3-portable.zip`：便携包
- `qingjian.exe`：主程序
- `SHA256SUMS.txt`：文件完整性校验值

数据默认保存在 `%LOCALAPPDATA%\com.qingjian.desktop`。轻笺不会联网同步，也不会收集遥测数据。

## 主要功能

- 便签新建、搜索、置顶、归档、回收站与批量操作
- 文件夹、标签和最近编辑视图
- 富文本编辑、字体与字号、任务列表、表格和图片
- 圆圈待办：点击空心圆完成事项，完成后显示对号并弱化文字
- 多种复制方式，包括纯文本、HTML 和源格式
- 自动保存、全局快捷键、托盘运行与开机启动
- 透明窗口边缘、1px 细边框和轻柔黑色阴影

## 快捷键

- `Ctrl+Alt+N`：直接打开上次使用的便签；没有可用便签时自动新建
- `Ctrl+N`：新建便签
- `Ctrl+S`：立即保存
- `Ctrl+Tab` / `Ctrl+Shift+Tab`：切换编辑器标签
- `Ctrl+W`：关闭当前编辑器标签
- `Ctrl+,`：打开设置
- `Esc`：保存并关闭编辑器

## 本地开发

需要 Node.js、Rust stable MSVC 和 Visual Studio 2022 C++ Build Tools。

```powershell
npm install
npm run check
npm test
npm run tauri dev
```

## 构建 Windows 版本

```powershell
.\scripts\build-windows.ps1
```

构建产物默认生成在 `src-tauri\target\release`，发布前应将最终文件复制到 `release/<版本号>`。
