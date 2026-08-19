# MHW:I 武器重定向设计

> 状态（2026-08-05）：WR-01 `design-complete`，WR-02A 与 WR-03A `completed`。14 类 family/part
> registry、source/target/model path parser、source closure、纯内存 catalog-source validator、
> 有界 MOD3/MRL3 preflight/pair compatibility 和纯 MRL3 transformer 已在 `hmm-games-mhw` 落地。
> 完整 bundled catalog、staging、InstallPlan/manifest 集成和 UI 仍未完成。

## 背景

MHW:I 武器 Mod 的安装目标不是一个展示名称，而是一个游戏资源根。候选输入的只读结构审计已确认：

- 目标根形如 `nativePC/wp/<family>/<main_id>`。
- 14 类 family 共有 603 个唯一目标根，但可以对应 3125 个展示名称。
- 同一目标根最多对应 48 个名称；这些名称必须是一个目标的 localization/alias，不能生成重复安装目标。
- `main_id` 同时存在普通形式和 `bs_` 形式，二者都是资源身份的一部分。

武器模型还存在主件、副件和 MOD3/MRL3 配对。公开格式源码进一步确认 MRL3 内含固定宽度 texture
path 字段，因此武器重定向不能照搬 Armor 的纯路径复制，也不能对二进制做宽泛字符串替换。

## 设计结论

首个闭环固定以下结论：

1. 武器使用独立 `MhwWeaponReplacementAdapter`，不复用或扩张
   `MhwArmorReplacementAdapter`。
2. 游戏路径、family、part 和 MRL3 规则只存在于 `hmm-games-mhw`。
3. v1 只允许同 family 重定向；跨 family 固定为 `unsupported`。
4. `.mod3` 只在已验证的 Iceborne 格式、已识别 part、完整 MOD3/MRL3 配对下按路径移动，字节不改。
5. `.mrl3` 固定为 `binary_transformer_required`；必须有界解析并只改已识别的 texture path 字段。
6. 未知版本、未知 part、未知扩展名、混合 family、多 source root 和不完整 pair 全部 fail closed。
7. 原始导入包保持只读；转换后的字节只写 sibling `.partial` staging，成功后原子发布。
8. 最终路径、transformer id/version 和 binding facts 必须进入 plan identity、manifest 和 recovery。
9. 完整 catalog 仍受 CAT-01 provenance/licensing 门禁约束；来源未明数据不得 bundled。

## 目标

- 定义唯一、稳定且与展示名无关的 weapon target identity。
- 定义 14 类 family、主资源编号和已知副件角色。
- 定义 source root、model asset、part 和 MRL3 内部引用的结构化 parser 边界。
- 明确 path-only、binary-transformer-required 和 unsupported 三类能力。
- 保留 InstallPlan、conflict、backup、manifest、rollback/recovery 和 game/profile 写锁。
- 为 WR-02、WR-03、WR-04 提供可直接测试的完成定义。

## 非目标

- 不提交 3125 个展示名称、603 个目标根或任何来源未明 weapon JSON。
- 不在 WR-01 实现 Rust parser、catalog、transformer、staging 或真实游戏写入。
- 不支持跨 family 模型转换、骨骼/动画转换、patch model 或来源未审计的 extra part。
- 不支持 `.ctc`、`.ccl`、`.efx`、`.sobj`、`.tex` 等尚未证明安全的资源转换。
- 不在前端解析 `nativePC/wp`、派生 part id 或修改 MRL3。
- 不开放 Production CLI 写入，也不改变 Gate A、Gate B 或 Gate C 的认证边界。

## 术语

| 术语 | 含义 |
| --- | --- |
| family | 14 类武器的稳定路径 token，例如 `one`、`swo`。 |
| main id | 资源根编号，语法为 `(bs_)?<family><NNN>`。 |
| part role | `main` 或 family 注册的副件角色，例如 `shield`、`sheath`。 |
| part id | 文件 stem，语法为 `(bs_)?<part_prefix><NNN>`。 |
| source root | 包内唯一的 `nativePC/wp/<family>/<main_id>`。 |
| target root | catalog 中玩家选择的目标资源根。 |
| model pair | 同一 part id 的 `<part_id>.mod3` 与 `<part_id>.mrl3`。 |
| path-only | 只改变结构化相对路径，源文件字节逐字节保持。 |
| transformer | 读取受控源字节、验证格式并生成确定性目标字节的纯 adapter 能力。 |
| source closure | 本次分析纳入的 source root、part、文件和内部引用的排序后稳定事实集合。 |

## 信任边界与证据

### 仓库内事实

- [装备 Catalog 候选数据治理](EQUIPMENT_CATALOG_GOVERNANCE.md) 是 stable ID、名称、状态和许可门禁的
  权威契约。
- [MHW:I 外观套装重定向设计](ARMOR_RETARGET_DESIGN.md) 是 InstallPlan、binding、staging、manifest
  和 target switch 的已认证参考，但 Armor 路径语法不是 Weapon 语法。
- 私有 ignored 候选输入只用于聚合结构审计。原始记录、名称和生成 artifact 不进入仓库。

### 公开源码行为证据

研究固定到以下 commit：

- `Dimcirui/Modding-Toolkit@26c78a2d77ee141f6088349336a6b560f0620ee6`
  - `weapon_data.py` 列出 14 个 family、已知默认副件和 model path 形态。
  - `batch_import.py` 按 part 配对 MOD3/MRL3，以便正确解析材质名。
- `AsteriskAmpersand/Material-Editing@4b66616193857839fe69c0509735a3e0ac79b2bd`
  - MRL3 texture record 含固定 `char[256] path`。
  - MOD3 template 含 header、offset、mesh、material name/index；未把资源路径建模为字段。
- `AsteriskAmpersand/MHW-Mod3-Toolbox@fb695ad71b3d9dd27d13f1cd119fd821d36f9231`
  - 作为 MOD3 工具行为的交叉参考。

上述公开仓库的 GitHub license metadata 均未提供可识别许可证。本项目只记录行为事实和固定链接，
不复制其代码、数据或 catalog。缺少许可证不由技术 validator 自动解释为可再分发授权。

## 模块边界

### `hmm-core`

继续保存游戏无关的：

- `ReplacementTarget` / `ReplacementBinding` / `ReplacementBindingSnapshot`。
- `ReplacementAnalysis` / `RetargetPlan` / `RetargetAction`。
- 最终 `InstallTargetPath`、plan identity、manifest 和 recovery facts。

WR-03 可以增加通用、向后兼容的 transformer invocation 与 adapter snapshot facts，但 core 不得知道
`wp`、family、MOD3、MRL3 或 texture path。

### `hmm-ports`

保留 `ReplacementCatalogProvider` 和 `ReplacementAdapter`。WR-03 增加窄的纯字节 transformer port；
port 只接受受控 bytes、通用 action facts 和版本化 invocation，不接受真实游戏绝对路径。

### `hmm-games-mhw`

新增独立模块，建议布局：

```text
weapon_retarget/
  mod.rs
  family.rs
  path.rs
  catalog.rs
  mrl3.rs
  retarget.rs
```

职责：

- family/part registry。
- source/target/model asset parser。
- versioned weapon catalog loader/search/resolver。
- MOD3/MRL3 signature、bounds 和 pair 校验。
- MRL3 texture path transformer。
- `MhwWeaponReplacementAdapter` 的分析和 plan 构造。

### `hmm-app`

- 在写锁外完成分析、catalog 查询、transform preflight、prerequisite 和 candidate plan。
- 通过 transformer port 与 staging materializer 编排，不解析 MHW 二进制。
- target switch 复用真正重装，不先独立卸载再安装。

### `hmm-infra`

- 读取 package file bytes。
- 在 sibling `.partial` 下调用已注入 transformer，再执行 contained atomic write。
- 完整成功后原子发布 staging，失败清理。
- 不包含 family、part、MRL3 offset 或路径改写规则。

### Tauri / 前端

- 后端提供 category、capability、catalog、analysis、warnings 和 preview。
- 前端只提交 game/Mod/profile/source/target/layer identity。
- 前端不拼路径、不派生 `sldNNN`、不判断 MOD3/MRL3 是否兼容。

## 14 类 Family Registry

`family` 和 `part_prefix` 都是 ASCII 小写稳定 token。下表只冻结有公开结构证据的默认副件；任何
`extra_parts`、patch model 或新 prefix 都要独立证据、设计更新和负测，不能由 catalog 任意放宽。

| Weapon | family | main prefix | v1 已知副件 role / prefix |
| --- | --- | --- | --- |
| Great Sword / 大剑 | `two` | `two` | 无 |
| Sword & Shield / 片手剑 | `one` | `one` | `shield` / `sld` |
| Dual Blades / 双剑 | `sou` | `sou` | `right` / `sou_r` |
| Long Sword / 太刀 | `swo` | `swo` | `sheath` / `saya` |
| Hammer / 大锤 | `ham` | `ham` | 无 |
| Hunting Horn / 狩猎笛 | `hue` | `hue` | 无 |
| Lance / 长枪 | `lan` | `lan` | `shield` / `sld` |
| Gunlance / 铳枪 | `gun` | `gun` | `shield` / `sld` |
| Switch Axe / 斩斧 | `saxe` | `saxe` | 无 |
| Charge Blade / 盾斧 | `caxe` | `caxe` | `shield` / `sld` |
| Insect Glaive / 操虫棍 | `rod` | `rod` | 无 |
| Bow / 弓 | `bow` | `bow` | 无 |
| Heavy Bowgun / 重弩炮 | `hbg` | `hbg` | 无 |
| Light Bowgun / 轻弩炮 | `lbg` | `lbg` | 无 |

“无”只表示 v1 registry 没有已证明的默认副件，不表示游戏永远不存在其他资源。

## Stable Identity 与名称

### Target ID

Target stable ID 完全复用 CAT-01，不建立第二套算法。对 weapon 目标：

```text
path_family  = wp/<family>
resource_path = nativePC/wp/<family>/<main_id>
id = mhw:weapon:<完整 SHA-256 小写十六进制>
```

Hash 输入仍是：

```text
hmm-mhw-equipment-candidate-v1\0mhw\0weapon\0<path_family>\0<lowercase_resource_path>
```

同一个规范化资源根在 `ReplacementTargetId` 和 `ReplacementSourceId` 两种强类型位置可以复用同一
文本 stable ID。source 分析不要求该资源已经存在于 bundled catalog，但必须按相同算法重算。

### Display / Alias

- display name、alias、locale、provenance、条目顺序和状态都不参与 ID。
- 同一路径的多个名称必须在候选审计阶段合并成一个 target 的 localized names/aliases。
- 同 locale 的 primary display name 必须由经过审核的候选记录显式给出，不能按输入顺序自动挑选。
- UI locale 解析顺序为当前 locale、`en`、再按 locale key 稳定排序后的第一项；只影响显示。
- `hidden` 保留但默认不进入普通搜索；`dummy` 阻断 bundled artifact。
- 旧人工 slug 只能进入 `legacy_ids` resolver；歧义 legacy id 必须使 catalog 加载失败。

## 路径 Schema

### Resource Root

```text
nativePC / wp / <family> / <main_id>
```

约束：

- `family` 必须存在于上表。
- `main_id` 必须匹配 `^(bs_)?<family>[0-9]{3}$`。
- `main_id` 中的 family 必须与路径 family 相同。
- `bs_` 是 identity 的一部分，不能在 normalize 时删除。

### Model Asset

```text
nativePC / wp / <family> / <main_id> / mod / <part_id> . <ext>
```

v1 固定：

- 精确 6 个路径段。
- `<ext>` 只接受小写 `mod3` 或 `mrl3`。
- main part 的 `<part_id>` 必须等于 `<main_id>`。
- 副件 `<part_id>` 必须匹配 `^(bs_)?<part_prefix>[0-9]{3}$`。
- 副件的 `bs_` 存在性和三位数字必须与 `<main_id>` 相同。
- 每个 part id 必须恰有一份 `.mod3` 和一份 `.mrl3`，大小写折叠后也唯一。

示例只使用人工编号：

```text
nativePC/wp/one/one001/mod/one001.mod3
nativePC/wp/one/one001/mod/one001.mrl3
nativePC/wp/one/one001/mod/sld001.mod3
nativePC/wp/one/one001/mod/sld001.mrl3
```

### Target Part 派生

只按结构化字段派生：

1. 从 target `<main_id>` 解析可选 `bs_`、family 和三位数字。
2. `main` role 直接使用 target `<main_id>`。
3. 副件 role 使用 target 的可选 `bs_` + registry part prefix + target 三位数字。
4. 只替换路径中的 `<main_id>` 段和精确 `<part_id>` filename stem。
5. `nativePC`、`wp`、family、`mod` 和扩展名保持不变。

禁止 `replace("001", "002")`、对整个路径替换 source id，或根据展示名猜 part。

## Source Analysis

分析输入是 package file identity 与逻辑相对路径，不访问真实游戏目录。

v1 source closure 必须满足：

- 恰好一个 weapon source root。
- 恰好一个 family，且所有 model asset 的 main id 相同。
- 至少一个已注册 part 的完整 MOD3/MRL3 pair。
- source root 内没有未知扩展名、未知 part、嵌套额外目录或大小写碰撞。
- package 中没有第二个 `nativePC/wp` root，也没有其他会随安装写入的 `nativePC` 混合 payload。
- readme、preview 和 importer 已分类为非安装 payload 的元数据不进入 closure。

一个 family 的默认副件整体缺失可以产生 `partial_part_set` warning；某个已经出现的 part 缺少 MOD3
或 MRL3 则是 blocker。这样允许 main-only 或副件-only 的窄模型替换，但不允许半个 binary pair。

## Target Compatibility

Target 必须同时满足：

- stable target id 存在于当前 versioned catalog，或由唯一 legacy id 解析。
- `target_type=weapon`。
- metadata `family` 与 source family 相同。
- metadata `path_family` 等于 `wp/<family>`。
- target main id 通过 registry parser。
- target 状态可选择；`hidden` 只允许显式高级查询，`dummy` 永不允许。
- catalog 声明的 transformer capability 与 runtime 已注册版本一致。

跨 family、source/target capability 不一致或 transformer version 不可用时，不签发 preview token。

## 资源能力分类

| 资源 | v1 分类 | 放行条件 | 内容处理 |
| --- | --- | --- | --- |
| 已验证 Iceborne `.mod3` | `path_only` | 已识别 family/part、完整 pair、支持的 header/version | 字节逐字节复制；只改目标路径 |
| 已验证 Iceborne `.mrl3` | `binary_transformer_required` | 完整 pair、bounds/path preflight、注册 transformer 版本 | 只改已解析 texture path 字段 |
| `.tex` | `unsupported` | 无 | 等待独立格式/引用闭包证据 |
| `.ctc` / `.ccl` | `unsupported` | 无 | 物理/碰撞语义未证明 |
| `.efx` / `.sobj` / patch model | `unsupported` | 无 | 需要独立 parser/transformer 设计 |
| 未知扩展名或未知二进制版本 | `unsupported` | 无 | fail closed |

“MOD3 path-only”不是对任意 `.mod3` 的声明。parser 必须先验证受支持的 Iceborne signature/version、
offset/count bounds 和 pair；不满足时仍是 `unsupported`。

## MRL3 Transformer 契约

Transformer 是纯函数，不接触文件系统：

```text
source bytes + sealed invocation + source/target facts
  -> transformed bytes + deterministic report
```

### Preflight

- 验证 magic、Iceborne signature、header version、file size、count 和 offset bounds。
- 对 count、单字段和总解析字节设置固定上限，拒绝整数溢出和重叠区间。
- 读取固定宽度 texture path 字段时要求明确 NUL terminator；拒绝控制字符、绝对路径、drive/UNC、
  `.`、`..`、空语义段和无法支持的编码。
- game-resource path parser 独立于 `InstallTargetPath`：先把每个非空引用解析为安全的通用游戏资源
  路径；允许显式 game-relative envelope 或可选 `nativePC/` envelope，但仍拒绝绝对路径、空段和
  traversal。只有逻辑段以 `wp/<family>/<main_id>/...` 开头时才进一步解析 weapon family/main/tail。
- 未指向 `wp` 或未指向 source root 的安全引用保持原样；不因它属于共享 texture root 就拒绝。
- 指向 source root 的引用只替换精确 family/main 段；同 family 下 family 段实际不变。
- tail 中若包含 source main/part token，只允许精确注册的 filename stem 映射；模糊子串命中即拒绝。

### Rewrite

- 只写 parser 返回的 texture path byte ranges。
- 保留未修改字段、material hash、resource binding、count、offset 和文件长度。
- 输出编码和 separator envelope 保持输入形式。
- 新路径必须在固定字段容量内，并保留 NUL terminator；禁止截断。
- 禁止对整个 bytes 做搜索替换，禁止正则替换二进制，禁止修改未知 padding。

### Postcondition

- 重新解析输出，并确认所有 header/count/offset/material facts 与输入一致。
- 确认每个 source-root texture reference 已按 sealed mapping 收敛到 target root。
- 确认除预先声明的 path byte ranges 外没有其他字节变化。
- 输出 SHA-256、transformer id/version 和 canonical mapping digest，供 plan/staging 复验；不输出路径值。

v1 transformer id 建议固定为 `mhw.weapon.mrl3-texture-path.v1`。实现前必须用完全人工构造的小型
binary fixture 锁定 header/signature/version；不能提交游戏原始 MRL3。

## Generic Transformer Invocation

现有 `RetargetStagingFile` 只能表达“读原字节并复制”。WR-03 需要向后兼容扩展：

```text
RetargetAction
  ...existing path and source/target facts
  content_transform?              // None 表示 byte-for-byte copy
    transformer_id
    transformer_version
    source_content_digest
    canonical_mapping_digest

ReplacementBindingSnapshot
  ...existing facts
  adapter_facts?
    schema_version
    adapter_id
    strategy_id
    strategy_version
    source_closure_digest
    part_set_digest
    transform_set_digest
```

要求：

- 新字段对旧 Armor manifest 默认 `None`，不得重写既有 Armor binding。
- invocation 必须进入 RetargetPlan/InstallPlan token hash、batch facts digest 和 recovery snapshot。
- adapter facts 是 core 不解释的版本化数据；core 只做大小限制、确定性序列化和 hash。
- transformer registry 由 runtime composition 按精确 id/version 注入；未知或重复注册失败启动/preview。
- staging 读取 source 后先重验 source digest，再调用 transformer；transform 失败时不发布 partial。

## 安装、切换与卸载

```text
sealed package revision (read-only)
  -> weapon analyze / binary preflight
  -> target resolve / RetargetPlan
  -> transform-aware staging in sibling .partial
  -> InstallPlan / conflict / prerequisite
  -> persist Planned recovery intent
  -> game/profile write lock and final sealed-facts verification
  -> backup / commit / atomic manifest
  -> Completed or rollback / RollbackRequired
```

- binary parse/transform 和 hash 在写锁外完成。
- 写锁内只重验 sealed digest、containment、manifest/recovery 和短 commit facts。
- conflict 基于 transformed staging 的最终目标路径。
- same-revision target switch 复用真正重装，原子替换旧 entry/binding/adapter facts。
- uninstall 只根据 manifest-owned final files 和 backup facts，不重新解析 package/MRL3。
- staging 不是安装事实，成功、失败、取消和 rollback 后都可清理重建。

## Manifest、Recovery 与 Audit

Manifest 必须足以在不读取 catalog 当前展示名的情况下确认安装事实：

- Mod/profile/revision-owned replacement binding。
- source/target stable id、main id、path family 和 retarget kind。
- adapter/strategy/transformer id 与 version。
- source closure、part set、transform set digest。
- 每个最终文件的 package provenance、target path、hash、size 和 backup。

Recovery 保存同一 snapshot。重启后若 runtime 缺少 manifest 所需 transformer version，不自动迁移或
猜测；状态显示 `recovery-required` 或受控 unsupported，并保留 manifest/backup。

Audit 记录稳定 id、task id、operation、transformer id/version、聚合 part/file count、result 和错误码。
不得记录完整本地路径、texture path、第三方 Mod 内容、展示名称或原始 binary 字段。

## Fail-Closed 错误分类

建议稳定码：

| Code | 条件 |
| --- | --- |
| `weapon_source_not_found` | 没有可识别 weapon source root。 |
| `weapon_multiple_source_roots` | 多 main id 或多 source root。 |
| `weapon_mixed_family` | 同包出现多个 family。 |
| `weapon_unknown_family` | family 不在 registry。 |
| `weapon_invalid_main_id` | main id 语法或 family 不一致。 |
| `weapon_unknown_part` | filename stem 不是注册 part。 |
| `weapon_incomplete_binary_pair` | part 缺 MOD3 或 MRL3。 |
| `weapon_mixed_install_payload` | source closure 外仍有会安装的 payload。 |
| `weapon_unsupported_resource` | 扩展名、patch/extra part 或版本不支持。 |
| `weapon_cross_family_target` | source/target family 不同。 |
| `weapon_binary_format_invalid` | magic/signature/count/offset/bounds 不合法。 |
| `weapon_binary_reference_unsafe` | MRL3 路径字段不安全或不可解析。 |
| `weapon_binary_reference_ambiguous` | tail 需要模糊替换才能映射。 |
| `weapon_binary_path_too_long` | 目标字段超出固定容量。 |
| `weapon_transformer_unavailable` | 精确 id/version 未注册。 |
| `weapon_transformer_output_invalid` | postcondition 或 changed-range 校验失败。 |
| `weapon_plan_stale` | catalog、source hash、closure 或 transformer facts 漂移。 |

错误 projection 只返回稳定码、聚合数量和可操作状态，不回显路径或 binary 值。

## 安全测试矩阵

所有测试只使用内存 bytes、temp directory、fake reader 和人工 fixture。

### WR-02: Catalog / Family / Path / Analysis

| 场景 | 预期 |
| --- | --- |
| 14 个 family 的普通与 `bs_` main id | 全部结构化解析，family/main 一致 |
| 未知 family、位数错误、family mismatch | 拒绝 |
| `/` 与 `\` 输入 | 规范比较结果一致，输出路径唯一 |
| 同路径多 locale/name | 一个 stable target，名称只作 alias/localization |
| stable ID 对名称、顺序、状态变化 | ID 不变 |
| main 与六类已知副件派生 | role、`bs_` 和三位数字正确 |
| unknown/extra part、patch model | 拒绝 |
| missing pair、duplicate extension、case collision | 拒绝 |
| multiple roots、mixed family、mixed nativePC payload | 拒绝 |
| legacy id 唯一/歧义 | 唯一解析；歧义使 catalog invalid |
| unauthorized/dummy candidate | 不生成 bundled artifact |

### WR-03: Binary / Staging / Install

| 场景 | 预期 |
| --- | --- |
| 人工有效 MOD3/MRL3 pair | MOD3 不变，MRL3 仅声明 path ranges 改变 |
| base/未知 version、损坏 offset/count、截断字段 | preflight 拒绝且不建 staging |
| unsafe/absolute/traversal/control texture path | 拒绝 |
| source-root ref、other-root ref | 前者结构化映射，后者逐字节保持 |
| 模糊 tail token、目标 path 超 255 bytes | 拒绝，不截断 |
| source digest 或 transformer version 漂移 | token/plan stale |
| transform 中途失败 | `.partial` 清理，无发布 staging |
| symlink/junction、case collision、target escape | 拒绝且不写 root 外 |
| conflict | 使用 transform 后最终路径检测 |
| commit failure / rollback success | temp game baseline 恢复，无 recovery |
| rollback failure / restart | 保留 RollbackRequired 与完整 adapter facts |
| target switch / restart / uninstall | 新 target 持久化，最终 exact baseline |
| task/audit writer degradation | 玩家文件事实不伪造 rollback；degraded 显式可见 |

### WR-04: Contract / UI / Windows Gate

| 场景 | 预期 |
| --- | --- |
| catalog/filter/alias | UI 只消费后端 target，不拼路径 |
| unsupported/mixed/cross-family | 无确认按钮，展示稳定可操作原因 |
| preview token stale | 要求重新 preview，不执行 |
| install -> restart | target、part count、manifest 状态持久化 |
| same revision target switch -> restart | 真正重装、旧 target 无残留 |
| uninstall -> restart | manifest 空、recovery 空、baseline 精确一致 |
| 1440x900、1366x768、1280x800、480x800 | modal/overlay 不被顶栏遮挡，可滚动、无截断/路径泄漏 |
| light/dark/system theme | preview/result 与应用主题同步 |

## 分阶段实施计划

### WR-02A: Family、Parser 与人工最小 Catalog

状态：`completed`，2026-08-05。该切片不依赖外部 catalog 权利，已实现：

- `WeaponFamily` / `WeaponPartRole` registry。
- resource root、model asset 和 source closure parser。
- target metadata/legacy resolver validator。
- 完全人工的 14-family parser fixture 与严格 versioned catalog-source 测试输入。
- stable ID、alias 和 fail-closed analysis tests。

14-family catalog 只由测试内存 JSON 提供；production code 没有 provider、bundled seed、文件系统 I/O
或真实游戏写入。聚焦测试 15/15、`hmm-games-mhw` crate 63 项与 doc-tests、受影响三 crate
all-targets clippy 均通过。完成不代表 603 个目标 catalog 已 bundled，也不签发可执行 `RetargetPlan`。

### WR-02B: 经过许可审核的完整 Catalog

只有候选输入满足 CAT-01 `bundled_eligible` 且 reviewer 实际核对证据后才可实施：

- 合并同路径多名称。
- 生成 versioned runtime artifact。
- 603 唯一路径、大小写唯一、legacy id、搜索隔离和加载性能测试。

在此之前保持 `blocked-external-data`，不得用私有 JSON 临时代替。

### WR-03A: Binary Parser 与 Transformer

状态：`completed`，2026-08-05。该切片已实现：

- 完全人工 MOD3/MRL3 binary fixture，不提交或读取真实游戏 binary。
- 256 MiB 单文件上限、checked count/offset/range 算术、受支持版本与 material/resource bounds。
- MOD3 material JAMCRC 与 MRL3 material hash 集合的精确 pair compatibility。
- 安全 game-resource reference parser 和 `mhw.weapon.mrl3-texture-path.v1` 纯 transformer。
- 精确 source/target root 与六类副件 mapping、changed-range postcondition、确定性 source/output/mapping
  SHA-256，以及不回显路径、offset、material name 或 binary 内容的稳定错误/report projection。

固定入口 9/9、`hmm-games-mhw` 72 项及 doc-tests、受影响三 crate all-targets clippy 均通过。
本切片没有文件系统/staging、runtime transformer registry、InstallPlan/manifest/recovery、Tauri/UI、
production catalog 或真实游戏写入；这些边界仍属于 WR-03B/WR-04。

### WR-03B: Staging、InstallPlan 与 Manifest

状态：`completed`，2026-08-05。该切片已实现：

- `hmm-core` 中有界、版本化的 `ContentTransformInvocation`、transformer identity 与
  `ReplacementAdapterFacts`；旧 Armor binding/manifest 继续省略可选 facts，JSON 行为兼容。
- `hmm-ports` 中按精确 id/version fail-closed 的 `ContentTransformerRegistry`，以及
  `hmm-infra` 中 transform-aware sibling `.partial` materializer；source/dependency/output/mapping
  SHA-256 均在发布前重验，失败清理且不发布半成品。
- final staged target 继续进入既有 conflict/InstallPlan；adapter/transform facts 进入 install plan hash、
  reinstall token、batch digest、manifest 与 recovery snapshot。
- runtime composition 只注册固定 `mhw.weapon.mrl3-texture-path.v1` version `1` transformer，未增加
  第二个按 `game_id` 路由的 MHW adapter，也未绕过 production catalog 门禁。
- install/reinstall Audit 只投影 adapter/strategy/transformer id/version 与聚合 part/file count，不记录
  digest、invocation 参数、texture path、staging/cache/sandbox 路径或 binary 内容。
- 人工 bytes/temp-root acceptance 已证明 first install -> JSON manifest restart -> same-revision target
  switch -> JSON manifest restart -> manifest uninstall，并逐字节恢复 exact baseline；既有 commit failure、
  rollback success、rollback failure/recovery-required 测试继续覆盖同一 manifest/recovery 事务。

受影响六 crate 的完整测试、all-targets clippy 与 weapon lifecycle integration test 已通过；fixture 只使用
人工 MOD3/MRL3 bytes、fake services 和临时目录。WR-03B 没有新增 Tauri/UI、production weapon catalog
或 Production 写入；这些边界仍属于 WR-04/WR-02B。

### WR-04: Tauri、UI 与 Windows Gate D

状态：`certified`，2026-08-06。实现、自动 contract/build/完整验证、响应式主题 smoke 与 disposable
Windows Sandbox Gate D 均已完成。

- 单一 MHW 聚合 adapter/catalog 已落地；Production 保持 Armor-only，显式 Sandbox 才加入两个完全人工
  `one001`/`one002` developer target。
- `HMM_SANDBOX_DATA_DIR` 同时选择人工 seed 与 install/reinstall/uninstall/recovery root admission；marker、
  目录身份、link/reparse 和 app-data/game containment 在每次写入前 fail closed 重验。
- `list_replacement_targets` 要求 `modId`，由后端分析 source 并按 target type/path-family 过滤；DTO
  移除原始 metadata，增加 `catalogScope`，UI 不展示 relative path 或 path-family。
- content-aware plan 通过受限 reader 读取受控 revision 的人工 MOD3/MRL3 bytes，并生成 sealed transform
  invocation；仍复用既有 InstallPlan/task/write lock/manifest/backup/rollback/recovery 链。
- Mod 详情已覆盖 analysis、兼容 target、initial preview/confirm、same-revision true reinstall target switch、
  installed target、result、cancel、stale token、blocking conflict 与刷新失败状态。
- typed API、typecheck、lint、前端测试、production build、Rust contract/adapter/runtime tests 与 all-targets
  clippy 已通过；完整 `verify.ps1` 退出码为 0，Tauri 188 passed / 1 ignored。

- 最终 artifact SHA-256 为 `156c42118c6620d803c1611397c55c1847ab782bb6505cd713c56a17398ea2af`。
  disposable Windows Sandbox 使用人工最小 fixture 完成安装 -> 重启 -> `one001 -> one002` target
  switch -> 重启 -> 卸载；三个 task 分别为 `install-1785952182807-1`、`install-1785953522595-0`、
  `install-1785955067791-0`，均有匹配的成功 Audit Log。
- 卸载后 manifest `entries=[]`、`replacement_bindings=[]`，Recovery Center 与 backup/recovery/
  reinstall-recovery/retarget-staging 均为 0；game tree 为 10 文件/316 bytes，路径、大小和 SHA-256
  与初始 baseline 精确一致。空的共享父目录不属于文件 baseline，也不擅自删除。
- light 主题覆盖 1440x900、1366x768、1280x800、480x800；dark 覆盖 1280x800、480x800；system
  覆盖 1366x768。modal 层级、窄屏滚动、warning、按钮与路径脱敏通过。证据 bundle 名为
  `hmm-wr04-gated-20260805-2315`，保存在仓库外，不提交截图、日志、人工 archive 或 AppData。
- 已知非阻断缺陷包括：空 NexusMods ID 显示 `null`、
  二进制不兼容错误未映射为具体文案、主题入口可发现性弱。Gate D 只认证 replacement 主链与既定
  视觉检查，不表示这些 UI 缺陷已关闭。原列于此的"技术型 Mod fallback 名称"与"宽度不超过
  1360px 时 `.window-tools` 被隐藏"两项已在 `0.1.0-alpha.0` 真机验收后修复。

## 停止条件与开放问题

以下任一条件阻止对应实现，但不允许降级为猜测：

- 完整 catalog 没有明确 provenance、许可和 reviewer 事实。
- 不能用人工 fixture 锁定受支持的 MOD3/MRL3 signature/version/bounds。
- MRL3 中出现无法结构化解释的 source-related path。
- 新 part/resource 只有路径猜测，没有格式或运行行为证据。
- transformer invocation 无法进入 plan/manifest/recovery 的稳定事实链。
- Windows Gate 使用真实游戏目录、真实存档或第三方 Mod 才能通过。

开放问题：

- 是否有可再分发、可审计的完整 weapon target 数据源。
- v1 之后哪些 extra part、patch model 或 texture payload 值得逐类支持。
- 是否需要为旧 transformer version 保留只读 recovery implementation，还是提供显式受控迁移。
- 完整 catalog 未到位时，继续只对人工最小 seed 开放 developer/Sandbox capability；Production 保持
  Armor-only，直到 WR-02B 许可门禁完成并经过独立 review。

## 固定参考

- [Modding-Toolkit weapon_data.py](https://github.com/Dimcirui/Modding-Toolkit/blob/26c78a2d77ee141f6088349336a6b560f0620ee6/games/mhwi/weapon_data.py)
- [Modding-Toolkit batch_import.py](https://github.com/Dimcirui/Modding-Toolkit/blob/26c78a2d77ee141f6088349336a6b560f0620ee6/games/mhwi/batch_import.py)
- [Material-Editing MRL3 parser](https://github.com/AsteriskAmpersand/Material-Editing/blob/4b66616193857839fe69c0509735a3e0ac79b2bd/mrl3/MaterialMrl3.py)
- [Material-Editing MOD3 template](https://github.com/AsteriskAmpersand/Material-Editing/blob/4b66616193857839fe69c0509735a3e0ac79b2bd/utils/ShaderExtractors/010%20Templates/MOD3.bt)
- [MHW-Mod3-Toolbox](https://github.com/AsteriskAmpersand/MHW-Mod3-Toolbox/tree/fb695ad71b3d9dd27d13f1cd119fd821d36f9231)
