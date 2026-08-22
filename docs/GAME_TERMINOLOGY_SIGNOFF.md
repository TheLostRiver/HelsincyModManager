# `game_terminology` 许可签核记录

本文档是 [EQUIPMENT_CATALOG_GOVERNANCE.md](EQUIPMENT_CATALOG_GOVERNANCE.md)「关于
`game_terminology` 的政策决定」所需的**独立 review 签核记录**。政策决定于 2026-08-21 记录；
本文档核对该决定的执行事实，签核后 WR-05 的第一个发版前置即告完成。

## 签核范围

| 分发物 | 位置 | 条目数 |
| --- | --- | --- |
| 防具 catalog artifact | `src-tauri/crates/hmm-games-mhw/data/mhw-armor-targets.v1.json` | 269 |
| 武器 catalog artifact（14 family 分片） | `src-tauri/crates/hmm-games-mhw/data/weapons/mhw-weapon-targets.<family>.v1.json` | 601 |
| 署名声明 | `NOTICE.md`「MHW:I 装备与武器名称」一节 | — |

**只覆盖名称文本**（display name / alias，中英日三语）、内部编号（`pl001_0000`、`bs_one001`
等）、资源相对路径（`nativePC/...`）与由路径派生的 stable ID。图标、模型、贴图、剧情文本或
任何其他游戏资产不在本签核范围内，仍按 `unknown` / `restricted` fail closed。

## 证据记录

> **注意**：带 provenance 的候选文件（`armor-data/generated/mhw-equipment-candidates.*.v1.json`）
> 及抓取脚本按维护者决定**不进版本管理**（本地目录，git 排除）。因此以下证据内容**原文抄录**，
> 保证审计链在仓库内可追溯，不依赖任何一台机器。

### 1. 候选 source 声明（两份候选文件同一条，逐字抄录）

```json
{
  "source_id": "mhw-ingame-equipment-names",
  "source_name": "MHW:I in-game equipment names",
  "source_url": "https://www.monsterhunter.com/world-iceborne/",
  "retrieved_at": "2026-08-21",
  "license": {
    "status": "game_terminology",
    "rights_holder": "Capcom Co., Ltd.",
    "usage": "nominative",
    "attribution": "Equipment names are trademarks and content of Capcom Co., Ltd. This project claims no rights in them and is not affiliated with or endorsed by Capcom.",
    "reviewed_by": "Helsincy",
    "reviewed_at": "2026-08-21"
  }
}
```

`game_terminology` 状态的必填字段（`rights_holder` / `usage: "nominative"` / `attribution` /
`reviewed_by` / `reviewed_at`）全部在位；每个 target 均通过 `source_ids` 引用该 source。

### 2. 技术门禁复验（2026-08-23 实测）

```powershell
cargo run -p hmm-games-mhw --example validate_equipment_candidates -- --require-bundled <candidate.json>
```

- 防具候选（`catalog_version=mhw-armor-v2`，269 targets）：退出码 `0`，`issues: []`，
  `bundle_blockers: []`。
- 武器候选（`catalog_version=mhw-weapon-v1`，601 targets）：退出码 `0`，`issues: []`，
  `bundle_blockers: []`。

### 3. 分发物字段清单（2026-08-23 全量扫描核对，269 + 601 条逐条）

武器分片每条 target 只包含：`stable_id`（`mhw:weapon:<SHA-256>`，由 resource_path 派生，
601 条格式全部匹配）、`target_type`、`resource_path`、`internal_id`、
`metadata.{family, path_family}`、`status`（601 条全部 `active`，dummy 已在生成前剔除）、
`names`（每条恰含 zh_cn/en/ja 三个 locale，各一个 display_name + aliases）、`legacy_ids`
（601 条全部为空）。

防具 artifact 的 schema 与武器分片不同，每条 target 只包含：`id`（`mhw:armor:<SHA-256>`，
269 条格式全部匹配）、`target_type`、`display_name`（per-locale 名称文本）、`aliases`
（名称文本）、`internal_id`（`plXXX_XXXX`）与 `metadata.{path_family, parts, variant,
monster, rank, is_full_body, legacy_ids}`；metadata 取值均为机器枚举/slug（如 `guardian`、
`alpha`、`high`，`metadata.legacy_ids` 为旧版内部 ID 如 `mhw:armor:guardian-alpha`），不是
本地化文本；无 `status`/`resource_path` 字段（防具走结构化 slot 改写，不携带资源相对路径）。

两类 artifact 均不含名称文本、内部标识与派生 stable ID 之外的任何游戏资产。

### 4. 边界核对（政策决定的四条边界，逐条验证）

| 边界 | 核对结果 |
| --- | --- |
| 名称只作为绑定在 retarget catalog 上的功能性标识 | 成立：名称仅在替换目标选择面板内展示与检索（`ReplacementTargetPanel`），用于把 Mod 资源对上官方槽位 |
| 不得做成可独立浏览、可导出的装备名称数据库产品 | 成立：前端 `replacements` feature 无任何数据导出功能（全量 grep 确认，`export` 命中均为 JS 模块关键字）；catalog 无独立浏览入口 |
| `rights_holder` 写明真实权利人，项目不主张权利 | 成立：候选声明与 NOTICE.md 均写明 Capcom Co., Ltd.，并明示不主张权利、无关联、未获认可 |
| 分发物必须带署名声明 | 成立：NOTICE.md 有专门一节，含指称性使用说明、商标声明与权利人联系渠道（SECURITY.md / issue） |

### 5. 署名声明（NOTICE.md 摘要核对）

NOTICE.md「MHW:I 装备与武器名称」一节内容与政策边界一致：权利人、指称性用途、不主张权利、
非关联方、商标归属、移除请求渠道均具备。

## Reviewer 核对清单

签核前 reviewer 应实际完成以下核对（各项均可独立复现）：

- [x] 通读 governance 文档「关于 `game_terminology` 的政策决定」一节，理解依据与四条边界。
- [x] 在本地运行上述 `--require-bundled` 验证命令（防具与武器各一次），确认退出码 0
      （2026-08-23 复跑，两次输出与第 2 节记录一致）。
- [x] 抽查 bundled artifact 若干条目，确认字段不超出第 3 节清单（含中英日名称的来源合理性
      抽查——例如 `one001` = 炎王纹章 / Teostra's Emblem / テオ＝エンブレム）。实际执行为
      269 + 601 条全量字段扫描，结果见第 3 节。
- [x] 确认 NOTICE.md 署名存在且与候选声明一致。
- [x] 确认第 4 节四条边界在当前产品形态下成立。

## 签核记录

| 项 | 值 |
| --- | --- |
| Reviewer | Helsincy |
| 签核日期 | 2026-08-23 |
| 覆盖 artifact 版本 | 防具 `mhw-armor-v2`（269 条）、武器 `mhw-weapon-v1`（601 条） |
| 决定 | ☑ 通过，发版前置解除 |

签核只对上表所列 artifact 版本有效。catalog 数据更新（新增条目或版本变更）时，本签核须
重新执行——候选 source 声明、`--require-bundled` 复验与边界核对缺一不可。
