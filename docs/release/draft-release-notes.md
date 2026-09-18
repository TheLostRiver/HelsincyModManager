此 draft 由 CI 自动生成，发布前必须完成人工验收。

- 验收清单：`docs/release/ALPHA_0_ACCEPTANCE.md`
- 变更说明：`CHANGELOG.md`

## 已知限制

- **无应用内自动更新**，后续版本需从 Releases 手动下载。说明见 `docs/release/UPDATER_PLAN.md`。
- 安装包**未经过代码签名**，Windows 会提示未知发布者。
- 仅 Windows x64 为支持平台。Linux / Steam Deck 未经实机验证，不作为支持平台。

## 使用前提醒

使用前建议自行备份游戏存档。

## 安装时被 Windows 拦截

安装包**未经过代码签名**，Windows 安全中心（SmartScreen）可能提示「Windows 已保护你的电脑」。
点击 **更多信息** → **仍要运行** 即可继续安装。

## 校验

下载后用同版本的 `SHA256SUMS-<版本号>.txt` 核对文件哈希。
