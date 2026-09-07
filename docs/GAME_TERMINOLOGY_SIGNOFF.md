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

## 重签记录：mhw-armor-v3（2026-08-24）

### 变更内容

防具 artifact 升至 `mhw-armor-v3`，为 5 条活动/联动装补齐缺失的名称文本：其中 4 条
（pl019_0000、pl057_0000、pl133_0000、pl132_0010）补齐 en/ja 展示名；pl057_0010
（男版燕尾蝶）补齐 ja 展示名，其官方英文名与 pl019_0000（女版）逐字同为 "Butterfly β"，
按本文档上游治理规则（同 locale display name 跨目标唯一；alias 可合理指向多目标）记为
en alias。最终统计：268/269 条持有三语展示名，1 条为 zh_cn/ja 展示名 + en alias。
条目数（269）、stable ID、resource 语义与武器 artifact（`mhw-weapon-v1`）均未变化。

### 名称权利与转录渠道

**名称权利人是 Capcom Co., Ltd.**（`game_terminology`，nominative use，与第 1 节候选 source
声明同一条：`mhw-ingame-equipment-names`，指向游戏内名称文本本身）。kiranico
（mhworld.kiranico.com）**仅为名称文本的转录/对照渠道，不是权利来源**——catalog 既有
zh_cn 名与 kiranico zh 系列页逐字一致（同源核对），新增 en/ja 名按同一系列页逐条转录，
冷僻日文名（パピメル/パピオム 系）另经 altema.jp/game8.jp 交叉验证。第 4 节四条边界不变。

### 复验证据（2026-08-24 实测）

- 候选 source 声明：从 v3 artifact 重建候选文档（269 条 resource_path 逐条经治理 Stable ID
  算法回验，269/269 命中），source 声明沿用第 1 节同一条，`retrieved_at`/`reviewed_at`
  更新为 2026-08-24。
- `cargo run -p hmm-games-mhw --example validate_equipment_candidates -- --require-bundled
  <上一条重建出的候选 JSON（本地临时产物，按维护者决定不入库）>`：退出码 `0`，
  `valid: true`，`bundled_eligible: true`，`issues: []`，`bundle_blockers: []`
  （269 targets，269 active）。
- 分发物字段清单：269 条全量扫描，target 键集恰为
  `{id, target_type, display_name, aliases, internal_id, metadata}`，metadata 键并集不超出
  第 3 节清单，`mhw:armor:<64 hex>` 格式 269/269 合规；不含名称文本、内部标识与派生
  stable ID 之外的任何游戏资产。
- 边界核对：第 4 节四条逐条复核成立（本变更仅改名称文本，前端与导出面零变更，
  NOTICE.md 署名一节未动且与候选声明一致）。
- 防回归：防具/武器 catalog 新增键集完备性测试（Butterfly β 重名例外单独锁定）。

### 签核

| 项 | 值 |
| --- | --- |
| Reviewer | Helsincy |
| 签核日期 | 2026-08-24 |
| 覆盖 artifact 版本 | 防具 `mhw-armor-v3`（269 条）；武器 `mhw-weapon-v1`（601 条，沿用 2026-08-23 签核） |
| 决定 | ☑ 通过（reviewer 批复"准许通过"，声明/复验/核对由协作 agent 按上表执行并留证） |

## 重签记录：mhw-armor-v4（2026-09-07）

### 变更内容

防具 artifact 升至 `mhw-armor-v4`，**269 → 529 条**：每件装备按它在游戏本体里**实际存在的
模型变体**产出目标，不再假设所有装备都有女性模型（260 个两套模型都有 ⇒ 各出 2 条，
4 个只有女性模型、5 个只有男性模型 ⇒ 各出 1 条；`#356`）。

分发物位置随之变化：单文件 529 条为 310KB / 12024 行，超出仓库 policy 的体积硬限
（256KB / 10000 行），artifact 因此按 `path_family` 拆成两份分片，与武器侧按 family 分片
同一做法：

| 分发物 | 位置 | 条目数 |
| --- | --- | --- |
| 防具 catalog artifact（f_equip 模型） | `src-tauri/crates/hmm-games-mhw/data/armor/mhw-armor-targets.f_equip.v1.json` | 264 |
| 防具 catalog artifact（m_equip 模型） | `src-tauri/crates/hmm-games-mhw/data/armor/mhw-armor-targets.m_equip.v1.json` | 265 |
| 武器 catalog artifact（14 family 分片） | `src-tauri/crates/hmm-games-mhw/data/weapons/mhw-weapon-targets.<family>.v1.json` | 601（沿用 2026-08-23 签核） |

**`f_equip` / `m_equip` 不是「女装／男装」，是同一件装备的两套模型。** 玩家的角色性别决定
游戏加载哪一套，因此两套都必须是合法目标；上游治理规则的修订见
[EQUIPMENT_CATALOG_GOVERNANCE.md](EQUIPMENT_CATALOG_GOVERNANCE.md) 的 Armor adapter 规则与
「名称与条目状态」两节（display name 唯一性从「跨目标唯一」收敛到「同一 `path_family` 内
唯一」——同一件装备的两个模型变体本来就同名）。

### 名称权利与来源

**权利人是 Capcom Co., Ltd.**（`game_terminology`，nominative use）。**来源就是游戏本体**：
名称文本取自游戏内的装备名称，变体归属取自本机安装的游戏资源目录枚举。

**本次升版未引入任何新的名称文本**（实测：529 条的 `display_name` 与 `aliases` 与 v3 同槽位
逐字相同，新增名称字符串 0 条，无新槽位）。529 条相对 269 条多出来的部分，全部是把**已有
名称**复制到同一件装备的第二个模型变体上。因此 2026-08-24 记录的名称来源与转录渠道说明
（`mhw-ingame-equipment-names`；kiranico 等仅为转录/对照渠道，不是权利来源）完整覆盖本版本
的全部名称文本，无需新增名称来源审核。

本版本唯一的新增数据是**模型变体归属**，声明为独立 source `mhw-game-assets`：它是结构事实
（游戏资源里哪个目录存在），与名称来源分开声明，不涉及任何第三方渠道。

### 复验证据（2026-09-07 实测）

**1. 候选 source 声明（逐字抄录，两条）**

```json
[
  {
    "source_id": "mhw-ingame-equipment-names",
    "source_name": "MHW:I in-game equipment names",
    "source_url": "https://www.monsterhunter.com/world-iceborne/",
    "retrieved_at": "2026-09-07",
    "license": {
      "status": "game_terminology",
      "rights_holder": "Capcom Co., Ltd.",
      "usage": "nominative",
      "attribution": "Equipment names are trademarks and content of Capcom Co., Ltd. This project claims no rights in them and is not affiliated with or endorsed by Capcom.",
      "reviewed_by": "Helsincy",
      "reviewed_at": "2026-09-07"
    }
  },
  {
    "source_id": "mhw-game-assets",
    "source_name": "MHW:I game assets (model variant enumeration)",
    "source_url": "https://www.monsterhunter.com/world-iceborne/",
    "retrieved_at": "2026-09-07",
    "license": {
      "status": "game_terminology",
      "rights_holder": "Capcom Co., Ltd.",
      "usage": "nominative",
      "attribution": "Model variant availability is enumerated from a local game installation. This project claims no rights in the game assets and is not affiliated with or endorsed by Capcom.",
      "reviewed_by": "Helsincy",
      "reviewed_at": "2026-09-07"
    }
  }
]
```

`game_terminology` 状态的必填字段（`rights_holder` / `usage: "nominative"` / `attribution` /
`reviewed_by` / `reviewed_at`）两条均在位；529 条 target 的 `source_ids` 同时引用两条
（变体归属对每一条目都成立，只引用名称那一条会让声明与数据脱节）。

**2. 技术门禁复验**

```
cargo run -p hmm-games-mhw --example validate_equipment_candidates -- --require-bundled <candidate.json>
```

退出码 `0`，`valid: true`，`bundled_eligible: true`，`issues: []`，`bundle_blockers: []`，
`target_count: 529`（`active: 529` / `hidden: 0` / `dummy: 0`）。候选文档从 v3 artifact 与
游戏本体变体枚举重新生成，`retrieved_at` / `reviewed_at` 更新为 2026-09-07。

**3. 分发物字段清单（529 条全量扫描）**

target 键集恰为 `{id, target_type, display_name, aliases, internal_id, metadata}`；
`metadata` 键并集为 `{path_family, parts, variant, monster, rank, is_full_body, legacy_ids}`，
**未超出第 3 节清单**；`mhw:armor:<64 hex>` 格式 529/529 合规；locale 并集恰为
`{zh_cn, en, ja}`，528/529 三语齐全（唯一例外仍是 `pl057_0010`，官方英文名与 `pl019_0000`
重名、按治理规则走 en alias）。不含名称文本、内部标识与派生 stable ID 之外的任何游戏资产。

**4. 身份链与存量绑定（本轮新增的复核项）**

- 529/529 条的 `id` 等于治理 Stable ID 算法对 `(armor, path_family, resource_path)` 的输出
  （新增测试 `armor_catalog_ids_match_the_governance_stable_id_algorithm` 常态钉住；武器侧
  一直在 loader 里重算，防具侧此前既无 loader 校验也无测试）。
- v3 的 273 个旧 ID（269 个 stable ID ＋ 4 个 AR1 手工 slug）经真实
  `find_replacement_target` 路径实测**全部仍可解析**，逐条恰好一个落点。玩家已安装的
  manifest / binding snapshot 因此不受本次升版影响。

**5. 边界核对（政策决定的四条边界，逐条复核成立）**

| 边界 | 核对结果 |
| --- | --- |
| 名称只作为绑定在 retarget catalog 上的功能性标识 | 成立：名称仅在替换目标选择面板内展示与检索，用于把 Mod 资源对上官方槽位 |
| 不得做成可独立浏览、可导出的装备名称数据库产品 | 成立：`src/features/replacements/` 无任何数据导出路径（全量 grep `download`/`toCSV`/`saveAs`/`writeFile`/`clipboard`，命中均为 JS 模块关键字或提示玩家「重新下载 Mod」的界面文案）；catalog 无独立浏览入口 |
| `rights_holder` 写明真实权利人，项目不主张权利 | 成立：两条候选声明与 NOTICE.md 均写明 Capcom Co., Ltd.，并明示不主张权利、无关联、未获认可 |
| 分发物必须带署名声明 | 成立：NOTICE.md「MHW:I 装备与武器名称」一节指向 `hmm-games-mhw/data/`，内容与候选声明一致，分片化后路径仍在其覆盖范围内 |

**6. 本轮复核中发现并修掉的四处缺陷**

签核复验不是走流程——第一次运行 `--require-bundled` 得到的是 `valid: false`、1310 条 issue。
逐条查证后修掉，才有上面的 `issues: []`：

| 缺陷 | 影响 | 处置 |
| --- | --- | --- |
| 治理 validator 的 armor 分支硬编码 `path_family != "pl/f_equip"` | 265 条 `wrong_path_family`：治理层拒绝 adapter 已经接受的全部 `m_equip` 数据。治理规则已在上游修订，validator 漏改 | 变体清单收敛到 `ArmorEquipFamily` 一处，validator 与 catalog 加载都从它派生 |
| validator 的 display name 唯一性仍按全局判定 | 780 条 `duplicate_display_name`：与「同一 `path_family` 内唯一」的治理修订脱节 | 唯一性键改为 `(path_family, locale, name)`；同一 family 内重名仍拒绝（正反用例各自具名钉住） |
| 候选文档声明了 `mhw-game-assets` 却无目标引用 | 1 条 `unused_source`：provenance 声明与实际数据脱节 | 529 条 target 的 `source_ids` 同时引用两条来源 |
| 264 条 `legacy_ids` 等于该条自己的 stable ID | 264 条 `legacy_id_matches_stable_id`：槽位 family 未变时新旧 ID 逐字节相同，写进 legacy 属冗余噪声 | 生成脚本按主 ID／legacy 两条解析分支判定落点，自指项不再写入；旧 ID 解析能力经实测未变（见第 4 项） |

**7. 防回归**

本轮新增／收紧的测试：artifact stable ID 与治理算法一致（529 条逐条）、两套模型变体均为
合法身份、display name 唯一性按 `path_family` 分组（含同 family 内重名仍拒绝的反向用例）、
不在册的 `pl/*` 变体仍报 `wrong_path_family`、磁盘分片与编译结果对账、跨分片冲突不因合并
而降级。生成脚本增加旧 ID 落位自校验与分片体积自校验，偏差时拒绝产出。

### 签核

| 项 | 值 |
| --- | --- |
| Reviewer | Helsincy |
| 签核日期 | 2026-09-07 |
| 覆盖 artifact 版本 | 防具 `mhw-armor-v4`（529 条，2 份 `path_family` 分片）；武器 `mhw-weapon-v1`（601 条，沿用 2026-08-23 签核） |
| 决定 | ☑ 通过（reviewer 授权以其名义核签；声明、`--require-bundled` 复验、字段清单、身份链与边界核对由协作 agent 按上表逐项执行并留证） |

签核只对上表所列 artifact 版本有效。catalog 数据更新（新增条目、版本变更或分片布局变化）时，
本签核须重新执行——候选 source 声明、`--require-bundled` 复验与边界核对缺一不可。
