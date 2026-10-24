此 draft 由 CI 自动生成，发布前必须完成人工验收。

- 验收清单：`docs/release/ALPHA_0_ACCEPTANCE.md`
- 变更说明：`CHANGELOG.md`

---

# Helsincy Mod Manager 0.1.2-alpha

第三个 Alpha 版本 · Third Alpha Release

---

## 中文说明

**版本**：`0.1.2-alpha`
**发布日期**：2026-10-24
**发布类型**：Alpha —— 主体功能已经可用、界面也已稳定；Alpha 阶段可能存在一些 Bug，欢迎反馈
**支持平台**：Windows x64（Tier 1）。Linux / Steam Deck 未经实机验证，不作为支持平台。

### 这是什么

Helsincy Mod Manager（HMM）是一款面向《怪物猎人：世界 冰原》（MHW:I）的 Windows 桌面
Mod 管理器：导入与管理 Mod、装备重定向，以及独立于 Mod 功能的多账号存档备份。

### 主要功能

- **精准锚定式新手引导**：直接高亮当前界面的真实按钮和功能区，随窗口尺寸与滚动位置更新定位。
- **Steam 游戏自动扫描**：自动查找 MHW:I 安装目录（也可手动指定），并检测前置文件、提示缺失项。
- **Mod 导入与管理**：支持 ZIP / RAR / 7z；单个、批量与拖拽导入，安装、卸载、删除均支持批量并给出逐项结果。
- **安装配置**：查看包内目录树，自行挑选要安装的文件，支持按目录批量选择、按名称或路径搜索。
- **武器、防具与猎虫重定向**：内置 1,159 个替换目标（武器 601 / 防具 529 / 猎虫 29），可先预览文件变更再应用。
- **分类、标签、筛选与排序**：按名称、作者或标签搜索，按分类与安装状态筛选，并记住排序选择。
- **Mod 卡片预览图**：从包内图片生成封面（PNG / JPG / JPEG / WebP），还可开启卡片悬浮信息查看详情。
- **从第三方 Mod 管理器迁移**：扫描狩技盒子来源目录、预览候选后批量导入，保留名称、作者与版本等信息；
  全程只读，原库不受影响。
- **更改 Mod 存储目录**：把 Mod 库迁到空间更充足的磁盘，迁移过程带校验。
- **存档目录自动扫描与多账号配置档**：自动发现本机各 Steam 账号的存档目录，一个账号一个配置档。
- **手动、自动与系统级后台备份**：支持手动备份与定时自动备份，HMM 完全关闭后仍可由 Windows 计划任务继续运行。
- **三语界面**：简体中文、日本語、English，可跟随系统语言。

功能细节见 [docs/PROJECT_FEATURES.md](https://github.com/TheLostRiver/HelsincyModManager/blob/main/docs/PROJECT_FEATURES.md)。

### 本版本更新

#### 游戏已经关闭，却仍提示「游戏正在运行」

部分玩家遇到过：游戏早已退出，HMM 仍判定游戏在运行，于是**拒绝安装、卸载、恢复与存档还原**
（批量安装结果显示 `install_blocked_game_running`），**自动存档备份也被一直延后**，直到重启电脑才恢复。

- 原因：Windows 的进程列表里可能残留一条与游戏同名、但实际早已停止运行（没有任何线程）的记录，
  实测可在游戏退出后滞留数天，直到重启电脑。旧版本只比对进程名，会把它当成游戏仍在运行。
- 现在：这类没有线程的残留不再算作游戏在运行。**游戏真正运行时仍会照常拦截写入**，
  包括残留还在、你又重新启动了游戏的情况。
- 本次修复不改动安装清单、备份格式与数据库结构，升级后无需重新扫描 Mod 库。

### 已知限制

- **无应用内自动更新**，后续版本需从 Releases 手动下载。
- 安装包**未经过代码签名**，Windows 会提示未知发布者。
- 仅 Windows x64 为支持平台。
- 后台备份需要电脑开机、用户已登录且系统计划任务正常；关机或未登录期间无法执行备份。
- 后台任务约每 15 分钟检查一次，**「下次执行时间」不等于准点执行**，游戏运行、休眠或任务占用都可能让它延后。
- Alpha 阶段可能存在一些尚未发现的 Bug；遇到问题欢迎通过 Issues 反馈。

### 使用前提醒

使用前建议自行备份游戏存档。

### 安装时被 Windows 拦截

安装包**未经过代码签名**，Windows 安全中心（SmartScreen）可能提示「Windows 已保护你的电脑」。
点击 **更多信息** → **仍要运行** 即可继续安装。

### 官方渠道声明

> [!WARNING]
>
> Helsincy Mod Manager 是**完全免费、开源**的桌面应用，**不会向用户收取任何费用**。
> 请只通过下列官方渠道获取本软件与更新：
>
> | 类别 | 官方 |
> | --- | --- |
> | 官网 | **[hmm.helsincy.com](https://hmm.helsincy.com)** |
> | 下载 | **[官网下载页](https://hmm.helsincy.com/download/)** · **[GitHub Releases](https://github.com/TheLostRiver/HelsincyModManager/releases)** |
> | 源码 | **[github.com/TheLostRiver/HelsincyModManager](https://github.com/TheLostRiver/HelsincyModManager)** |
> | 作者 | **[@TheLostRiver](https://github.com/TheLostRiver)** |
> | 问题反馈 | **[GitHub Issues](https://github.com/TheLostRiver/HelsincyModManager/issues)** |
>
> **本项目的下载渠道只有这两个：GitHub Releases 与官网下载页。**
> 任何向你收费、要求充值、索取登录凭据，或打着「加速下载」「绿色版」「破解版」旗号的第三方
> 站点与客户端，都不是官方发布。如果你是从其他来源拿到的，请停止使用，并按上方渠道重新下载。

### 校验

下载后用同版本的 `SHA256SUMS-0.1.2-alpha.txt` 核对文件哈希：

```text
<CI 产物生成后填入 / filled in after the CI build>  HelsincyModManager-0.1.2-alpha-windows-x64-setup.exe
```

---

## English

**Version**: `0.1.2-alpha`
**Release date**: 2026-10-24
**Release type**: Alpha — core features are usable and the UI is stable; expect some bugs during the Alpha stage
**Supported platform**: Windows x64 (Tier 1). Linux / Steam Deck are unverified and not supported.

### What this is

Helsincy Mod Manager (HMM) is a Windows desktop mod manager for Monster Hunter: World
Iceborne (MHW:I): importing and managing mods, equipment retargeting, and multi-account save
backups that stay independent of mod management.

### Key features

- **Anchor-based onboarding tour**: highlights the real buttons and panels of the current screen and keeps
  its position in sync as the window resizes or scrolls.
- **Automatic Steam game scan**: finds your MHW:I install directory automatically (manual selection is also
  supported), and checks for missing prerequisite files.
- **Mod import and management**: ZIP / RAR / 7z; import one, many, or by drag-and-drop. Install, uninstall,
  and delete all support batch operation with per-item results.
- **Install configuration**: browse the archive's directory tree and pick which files to install, with bulk
  selection by folder and search by name or path.
- **Weapon, armor, and kinsect retargeting**: 1,159 built-in retarget targets (601 weapons / 529 armor /
  29 kinsects), with a file-change preview before applying.
- **Categories, tags, filtering, and sorting**: search by name, author, or tag; filter by category and install
  state; your sort choice is remembered.
- **Mod card previews**: covers generated from images inside the archive (PNG / JPG / JPEG / WebP), plus an
  optional hover panel with details.
- **Migration from third-party mod managers** (e.g. 狩技盒子): scan the source directory, preview candidates,
  and bulk-import while preserving names, authors, and versions. The source is read-only and left untouched.
- **Change the mod storage directory**: move your mod library to a larger drive, with verification during migration.
- **Save directory auto-discovery and multi-account profiles**: discovers the save directory of each Steam
  account on the machine; one profile per account.
- **Manual, scheduled, and system-level background backups**: manual and scheduled automatic backups, plus
  backups that keep running via a Windows scheduled task after HMM is fully closed.
- **Trilingual UI**: 简体中文, 日本語, English, with an option to follow the system language.

See [docs/PROJECT_FEATURES.md](https://github.com/TheLostRiver/HelsincyModManager/blob/main/docs/PROJECT_FEATURES.md) for details.

### Updated in this release

#### "Game is running" no longer appears after the game has closed

Some players found that HMM still considered the game running long after it had exited, so it
**refused to install, uninstall, recover, or restore saves** (batch install results showed
`install_blocked_game_running`), and **automatic save backups kept getting postponed** until the PC
was restarted.

- Cause: the Windows process list can keep a leftover entry with the game's name that has in fact
  stopped running (it has no threads left). It was observed to linger for days after the game exited,
  until a reboot. Earlier versions matched on the process name only and mistook it for the running game.
- Now: such thread-less leftovers no longer count as the game running. **HMM still blocks writes while
  the game is actually running**, including when a leftover is present and you start the game again.
- This fix does not change the install manifest, backup format, or database schema, so there is no need
  to rescan your mod library after upgrading.

### Known limitations

- **No in-app auto-update**; future versions must be downloaded manually from Releases.
- The installer is **not code-signed**; Windows will warn about an unknown publisher.
- Only Windows x64 is supported.
- Background backups require the PC to be on and the user logged in with scheduled tasks working. Nothing
  runs while the machine is off or nobody is logged in.
- The background task checks roughly every 15 minutes, so the **"next run" time is not a promise of an
  exact start** — a running game, sleep, or another task can delay it.
- You may run into undiscovered bugs during the Alpha stage. Please report them via Issues.

### Before you use

We recommend backing up your game saves before use.

### Blocked by Windows during install

The installer is **not code-signed**, so Windows SmartScreen may show "Windows protected your PC."
Click **More info** → **Run anyway** to continue.

### Official download channels

> [!WARNING]
>
> Helsincy Mod Manager is a **free, open-source** desktop application.
> **We never charge you anything.** Get the app and its updates only from the official
> channels below:
>
> | Category | Official |
> | --- | --- |
> | Website | **[hmm.helsincy.com](https://hmm.helsincy.com)** |
> | Download | **[Download page](https://hmm.helsincy.com/download/)** · **[GitHub Releases](https://github.com/TheLostRiver/HelsincyModManager/releases)** |
> | Source | **[github.com/TheLostRiver/HelsincyModManager](https://github.com/TheLostRiver/HelsincyModManager)** |
> | Author | **[@TheLostRiver](https://github.com/TheLostRiver)** |
> | Issues | **[GitHub Issues](https://github.com/TheLostRiver/HelsincyModManager/issues)** |
>
> **These are the only two download channels: GitHub Releases and the official website.**
> Any site or client that charges you, asks for top-ups or login credentials, or advertises a
> "faster download", "portable build", or "cracked version" is not an official release.
> If you got the app from anywhere else, stop using it and download it again from the channels above.

### Verification

Verify the downloaded file against the matching `SHA256SUMS-0.1.2-alpha.txt`:

```text
<CI 产物生成后填入 / filled in after the CI build>  HelsincyModManager-0.1.2-alpha-windows-x64-setup.exe
```
