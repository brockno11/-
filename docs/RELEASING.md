# 轻笺发布与版本命名

## 版本怎么命名

轻笺使用 `主版本.次版本.修订版本`，判断依据是改动对用户、数据和兼容性的影响，不使用下载文件大小决定版本号。

- `0.5.0 → 0.5.1`：问题修复、性能优化、文案或小范围 UI 调整，不增加新的主要使用流程。
- `0.5.1 → 0.6.0`：新增向后兼容的主要功能，或明显改变一段产品体验。
- `0.x → 1.0.0`：产品达到稳定正式版，或发生需要迁移说明的不兼容变化。
- `0.6.0-beta.1`：公开测试版；带连字符的标签会自动发布为 GitHub 预发布版。

## 首次配置 GitHub 更新签名

应用内更新必须验证签名。当前公开公钥已写入 `src-tauri/tauri.conf.json`，对应私钥仅保存在本机：

`C:\Users\EDY\.tauri\qingjian.key`

私钥密码保存在 `C:\Users\EDY\.tauri\qingjian.key.password`。两者必须一起备份，并限制为当前 Windows 用户访问。

务必把私钥安全备份到密码管理器或离线介质。不要提交到 Git。若私钥丢失，现有安装无法验证后续更新。

在 GitHub 仓库的 `Settings → Secrets and variables → Actions` 新增：

- `TAURI_SIGNING_PRIVATE_KEY`：私钥文件的完整内容。
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`：密码文件的完整内容。

## 发布正式版

1. 更新 `CHANGELOG.md`，把待发布内容移动到对应版本。
2. 同步修改 `package.json`、`src-tauri/tauri.conf.json` 和 `src-tauri/Cargo.toml` 的版本号。
3. 运行 `npm run version:check`、`npm run check`、`npm test` 和 `cargo test`。
4. 提交代码后创建与版本一致的标签，例如 `git tag v0.5.0`。
5. 推送代码和标签。标签会触发 `.github/workflows/release.yml`。

工作流会创建公开 GitHub Release，并上传 Windows NSIS 安装包、更新签名和 `latest.json`。应用内检查更新固定读取：

`https://github.com/brockno11/-/releases/latest/download/latest.json`

正式下载入口：`https://github.com/brockno11/-/releases/latest`

历史版本入口：`https://github.com/brockno11/-/releases`
