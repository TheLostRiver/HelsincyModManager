此 draft 由 CI 自动生成，发布前必须完成人工验收。

- 验收清单：`docs/release/ALPHA_0_ACCEPTANCE.md`
- 变更说明：`CHANGELOG.md`

## 已知限制

- **无应用内自动更新**，后续版本需从 Releases 手动下载。说明见 `docs/release/UPDATER_PLAN.md`。
- 安装包**未经过代码签名**，Windows 会提示未知发布者。
- 仅 Windows x64 为支持平台。Linux / Steam Deck 未经实机验证，不作为支持平台。

## 安装前提醒

本工具会写入游戏目录并操作存档。Alpha 阶段建议：

- 先自行备份游戏目录与存档，不要只依赖本工具的备份功能。
- 不要在正在进行的重要存档上首次试用。

## 校验

下载后用同版本的 `SHA256SUMS-<版本号>.txt` 核对文件哈希。
