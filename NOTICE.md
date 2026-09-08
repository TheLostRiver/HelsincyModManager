# 第三方内容声明

本文件声明 Helsincy Mod Manager 分发物中包含的第三方内容及其权利归属。
本项目自身的代码与文档不在本文件覆盖范围内。

## MHW:I 装备与武器名称

`src-tauri/crates/hmm-games-mhw/data/` 下的 catalog artifact 包含《怪物猎人：世界 冰原》
（Monster Hunter: World — Iceborne）的装备名称。

- 权利人：Capcom Co., Ltd.
- 用途：指称性使用。这些名称仅作为绑定在重定向 catalog 上的功能性标识，
  用于把 Mod 资源对应到官方装备槽位，使玩家能识别自己正在替换哪一套装备。
- 本项目对这些名称**不主张任何权利**，不是卡普空的关联方，未获其认可或赞助。
- Monster Hunter、Monster Hunter: World、Iceborne 及相关名称是 Capcom Co., Ltd.
  的商标或注册商标。

治理契约与政策依据见
[docs/EQUIPMENT_CATALOG_GOVERNANCE.md](docs/EQUIPMENT_CATALOG_GOVERNANCE.md)
的「关于 `game_terminology` 的政策决定」一节。

## UnRAR（RAR 解压）

`src-tauri/crates/hmm-unrar-sys/vendor/unrar/` 是 **UnRAR 7.23（2026-06-27）源码的原样副本**，
用于支持 `.rar` 压缩包的导入解压。

- 权利人：Alexander L. Roshal
- 许可原文随源码一同保留在
  [`src-tauri/crates/hmm-unrar-sys/vendor/unrar/license.txt`](src-tauri/crates/hmm-unrar-sys/vendor/unrar/license.txt)
- 用途：**只解压，不压缩**。本项目不实现、不分发任何 RAR 压缩能力
  （这既是产品上的非目标，也是下述许可条款的硬性要求）
- 该目录**原样引入、不得就地修改**；若日后为构建适配必须修改，改动处须按许可要求
  在源码注释中带上下面第 2 条的全文

许可第 2 条要求其全文进入 license，无 license 文件时进入 documentation。本仓库当前
没有 LICENSE 文件，因此全文照录于此：

> UnRAR source code may be used in any software to handle RAR archives
> without limitations free of charge, but cannot be used to develop
> RAR (WinRAR) compatible archiver and to re-create RAR compression
> algorithm, which is proprietary. Distribution of modified UnRAR source code
> in separate form or as a part of other software is permitted, provided that
> full text of this paragraph, starting from "UnRAR source code" words,
> is included in license, or in documentation if license is not available,
> and in source code comments of resulting package.

第 4 条的免责声明：

> THE RAR ARCHIVER AND THE UnRAR UTILITY ARE DISTRIBUTED "AS IS".
> NO WARRANTY OF ANY KIND IS EXPRESSED OR IMPLIED. YOU USE AT
> YOUR OWN RISK. THE AUTHOR WILL NOT BE LIABLE FOR DATA LOSS,
> DAMAGES, LOSS OF PROFITS OR ANY OTHER KIND OF LOSS WHILE USING
> OR MISUSING THIS SOFTWARE.

接入方式、威胁模型与安全缓解见
[docs/ARCHIVE_FORMAT_SUPPORT_DESIGN.md](docs/ARCHIVE_FORMAT_SUPPORT_DESIGN.md)。

如果权利人希望本项目移除或调整上述内容，请通过
[SECURITY.md](SECURITY.md) 或仓库 issue 联系维护者。
