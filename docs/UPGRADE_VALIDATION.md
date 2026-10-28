# 0.8.0 升级验收

本次是开发候选，未创建标签、签名安装包或发布。原有未提交修改保留在工作区。
本地报告与截图保存在 `.reviews/2026-10-27-upgrade/`，它们不进入发布包。

## 必须检查的路径

- 正文最后一次输入后立即隐藏、归档、回收、退出、安装更新、导出完整备份；包含隐藏会话、保存失败、超时及迟到确认。
- 更新等待期间用托盘/快捷键显示便签，确认原生门禁不会创建新的可编辑会话。
- 长正文后段检索；搜索失败可重试；正文保存不覆盖标签和列表置顶。
- 旧数据库迁移、旧备份导入、同名标签/文件夹映射、失败导入后再次导入、导入窗口隐藏状态。
- 280×160最小卡片、404×704工作台、展开编辑、胶囊；焦点、快捷键、菜单返回及阅读位置。
- 圆圈12/36px及自动尺寸，混合字号、多行、嵌套、空待办、中文输入、撤销、图片、表格、复制与导出。
- 隐藏60秒后保存成功才释放；重新打开恢复内容和阅读位置，失败保留会话。

## 质量命令

```powershell
npm run quality:static
npm run quality:release
```

只有 `release/current/qingjian.exe` 是这次正式候选时运行 `npm run qa:windows`。
旧产物或隔离构建不能冒充正式验收；缺失前置条件时记录 `SKIPPED`。

## 独立数据的 Windows 构建

`scripts/prepare-isolated-build.py` 将已构建前端和Rust源码复制到指定 `.reviews/` 子目录，
只在该副本修改应用数据标识、单实例名称，并加入本地计时探针。它不修改正式构建的联网行为或应用界面。

```powershell
npm run quality:static
python scripts/prepare-isolated-build.py --output .reviews/my-run/source --label upgrade
# 在输出的snapshot目录调用仓库node_modules中的Tauri CLI，执行 build --no-bundle。
```

运行前使用 `scripts/qa-fixtures.py` 初始化 `com.qingjian.qa.*` 专用目录。
fixture工具拒绝普通应用目录以及缺少专用标记的已有目录；只对可丢弃的测试记录进行重置。
资源脚本还要求EXE旁的 `.qa.json` 记录匹配隔离标识和SHA256，拒绝普通应用或不匹配的构建。

```powershell
./scripts/measure-windows-resources.ps1 -ExecutablePath .reviews/my-run/qingjian.exe `
  -DataIdentifier com.qingjian.qa.upgrade -OutputPath .reviews/my-run/resources.json
```

同一台机器、相同发布优化模式、相同样例下记录0/1/5/10张便签。
内存是整个进程树工作集与私有字节之和；共享页面可能重复计数，不能称作唯一物理占用。
CPU按单逻辑核百分比记录。计时探针仅向本地 `.reviews/` 写文件，不收集或发送真实便签。
启动探针以首个可编辑DOM出现为代理，不等同于系统窗口已可见；输入以输入事件至两帧后的时间为代理。
唤回以窗口状态更新时间至两帧后为代理，均须注明探针与后台计时的局限。
正式的10%延迟阈值需要多次同条件样本；单次资源采样不构成性能达标证明。

启动对比使用 `scripts/compare-windows-startup.ps1`，两个隔离构建必须绑定同一个QA identifier和数据目录。
先分别测全新WebView缓存（不清操作系统缓存），再预热两个构建并交替AB/BA至少五组；每轮记录runId、预期便签集合完整性，并等待QA子进程退出。
旧缓存只移动到本次忽略的证据目录，不删除。输出目录必须为空，避免覆盖既有证据。
`--mode qa` 才包含前端阶段打点，原生阶段探针只注入隔离源码；正常生产构建不含探针。

```powershell
./scripts/compare-windows-startup.ps1 -Before .reviews/run/before.exe -After .reviews/run/after.exe -OutputDirectory .reviews/run/comparison
```

## 兼容与恢复

进展清单复用正文 JSON：`doc.attrs.progressEnabled` 缺省关闭，任务的 `checked` 决定完成，
`progressState` 保留最近的未完成状态。关闭进展不删除状态；旧文档默认待开始。
Markdown、纯文本和富文本复制、HTML 导出附带可读状态，回粘时剥离专用说明标签并保留任务属性。
自动摘要不写入正文、标题或全文检索。较旧编辑器重新保存时可能丢弃不认识的进展属性，
回退前应保留当前完整备份；本轮未改变 schema 6。

schema 6增加单张外观和阅读位置。迁移前自动备份原数据库；旧文档缺省继承全局外观。
完整备份采用独立一致性快照；合并导入保留本地已有便签，仅新增内容，新增窗口默认隐藏。
空闲回收后会重建编辑器；内容与阅读状态保留，撤销栈不跨回收保留。卡片/完整编辑切换继续共用编辑器。
本版可以读取旧备份；旧版应用不能读取新版完整备份。需要退回旧版时，应先退出应用，
使用迁移前快照或旧版备份恢复到独立目录验证，保留现有数据库和图片资源。

真实更新下载/签名安装、Windows 10/11、100%/150%/200%系统DPI和多显示器需要相应环境实测；
浏览器尺寸模拟不能替代这些检查。最终执行状态以本轮审阅报告为准。
