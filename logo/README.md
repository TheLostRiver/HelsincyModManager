# 应用品牌资源

- `HMM-logo.png`：用户提供的 1024x1024 透明母版的仓库优化版本，保持原画布与构图。
- `HMM-icon.png`：从母版的非透明内容边界居中生成，并保留约 6% 四周安全边距，供小尺寸桌面图标使用。

当前 Tauri 桌面图标由 `HMM-icon.png` 生成。重新生成时先输出到临时目录，再更新仓库现有的
`src-tauri/icons/icon.ico` 与 `src-tauri/icons/icon.png`：

```powershell
cmd /c corepack pnpm tauri icon logo\HMM-icon.png --output .tmp\app-icons
Copy-Item .tmp\app-icons\icon.ico src-tauri\icons\icon.ico
Copy-Item .tmp\app-icons\128x128@2x.png src-tauri\icons\icon.png
Copy-Item .tmp\app-icons\128x128@2x.png public\branding\hmm-logo.png
```

使用 256x256 的 `128x128@2x.png` 作为桌面 PNG 和应用内资源；默认生成的 512x512 `icon.png`
会超过仓库单文件字节限制，不应直接提交。

`src-tauri/build.rs` 显式跟踪这两个文件；否则 Cargo 增量构建可能继续复用旧的 Windows
`resource.lib`，使新图标没有进入可执行文件。

应用内 sidebar 使用 `public/branding/hmm-logo.png` 的 256x256 优化版本，避免为 36-52 px 展示加载
完整母版。
