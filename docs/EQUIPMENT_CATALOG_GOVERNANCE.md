# 装备 Catalog 候选数据治理

本文档定义 CAT-01 的候选输入契约、MHW:I 语义校验、稳定身份、来源/许可门禁，以及候选数据进入
bundled runtime catalog 前必须满足的边界。

## 目标与非目标

CAT-01 交付可信的数据入口，不交付完整装备数据：

- 候选 JSON Schema 与严格 Rust DTO。
- MHW:I adapter 内的路径族、资源身份、去重和名称校验。
- 与展示名、alias、输入顺序无关的确定性 stable ID。
- `active`、`hidden`、`dummy` 条目策略。
- provenance/licensing 证据和 bundled eligibility 门禁。
- 只读开发者验证入口与人工最小负测。

本切片不提交来源未明的 272 条防具路径、3125 个武器展示名或 603 个武器目标路径，不生成完整
bundled catalog，不修改 AR1-AR5 的安装、manifest、backup、rollback/recovery 或 staging 链路。

## 两层信任边界

```text
untrusted candidate JSON
  -> JSON Schema / strict serde shape
  -> MHW semantic validator
  -> valid candidate report
  -> provenance/licensing 人工核验
  -> bundled eligibility gate
  -> 独立提交生成并审查 runtime artifact
```

`valid=true` 只表示候选结构和语义可审计；不等于允许再分发。只有
`valid=true && bundled_eligible=true`，且 reviewer 已核对证据内容，后续生成器才允许消费该输入。
Schema/validator 与经过审计的生成 artifact 必须保持两个独立提交边界。

## 契约位置

- JSON Schema：`src-tauri/crates/hmm-games-mhw/data/schemas/mhw-equipment-candidates.v1.schema.json`
- Rust validator：`src-tauri/crates/hmm-games-mhw/src/equipment_catalog_candidate.rs`
- 只读入口：`src-tauri/crates/hmm-games-mhw/examples/validate_equipment_candidates.rs`

顶层固定字段：

| 字段 | 规则 |
| --- | --- |
| `schema_version` | 当前只接受整数 `1`；未知版本在读取 envelope 后 fail closed |
| `catalog_version` | 小写安全 slug，与 runtime catalog 版本相互独立 |
| `game_id` | 必须为 `mhw` |
| `sources[]` | 非空 provenance/license 记录；每条必须被至少一个 target 引用 |
| `targets[]` | 非空唯一资源目标；一个目标可以有多个 locale 名称和 alias |

所有 DTO 使用 `deny_unknown_fields`。JSON Schema 能表达字段形状；跨记录碰撞、stable ID 重算、MHW
路径族和许可门禁由 Rust validator 负责。

## 资源身份与路径

`resource_path` 是以 `nativePC` 开头的逻辑相对目标路径，不是本机绝对路径。validator 复用
`InstallTargetPath` 拒绝空路径、绝对路径、Windows drive/UNC 前缀、`.`、`..`、空段和非
`nativePC` root。分隔符先按 core 规则规范化，再要求输入等于规范化结果，避免同一资源存在多种编码。

CAT-01 固定以下 adapter 规则：

- Armor：`target_kind=armor`、`path_family=pl/f_equip`，目标根必须严格为
  `nativePC/pl/f_equip/plNNN_VVVV`。
- Weapon：`target_kind=weapon`、`path_family=wp/<family>`，路径必须位于
  `nativePC/wp/<family>/...` 且 family 一致。
- 路径段只允许 ASCII 字母、数字、`.`、`_`、`-`；大小写折叠后的路径必须全局唯一。

14 个 weapon family 的枚举、资源编号 parser 和二进制 transformer 分类已由
[MHW:I 武器重定向设计](WEAPON_RETARGET_DESIGN.md) 冻结；CAT-01 仍只负责不依赖具体 family 清单的
安全治理入口。

## Stable ID

Stable ID 只来自资源身份，display name、alias、locale、provenance、条目状态和数组顺序都不参与。
算法固定为：

1. 按上述规则验证并得到规范化 `resource_path`。
2. 将路径按 ASCII 小写用于 Windows 大小写不敏感身份比较；`path_family` 必须已是小写规范形式。
3. 以 UTF-8 编码下列 NUL 分隔串：

   ```text
   hmm-mhw-equipment-candidate-v1\0mhw\0<target_kind>\0<path_family>\0<lowercase_resource_path>
   ```

4. 计算完整 SHA-256 小写十六进制，生成
   `mhw:<target_kind>:<64 lowercase hex>`。

validator 必须重算并拒绝不匹配、重复 stable ID、重复路径和大小写碰撞。完整 256-bit digest 不截断，
避免不同路径被人类 slug 规则压成同一 ID。

现有四个 AR1 target ID 不被重写。候选条目可在 `legacy_ids[]` 声明旧 ID；AR6 生成/加载边界必须据此
提供兼容 resolver，并增加旧 binding ID 回归测试。`legacy_ids` 不是新的 stable identity，也不能重复。

## 名称与条目状态

`names` 按 locale 保存：每个 locale 有一个 `display_name` 和零个或多个 `aliases`。名称使用现有 adapter
的 NFKC、大小写、空白和中点归一化规则比较。同一 locale 的规范化 display name 不得跨目标重复；
alias 只用于显示/检索，不参与 stable ID。同一 alias 可以合理地指向多个目标，例如同一怪物的
Alpha/Beta 条目。

条目状态：

| 状态 | 含义 | Bundling |
| --- | --- | --- |
| `active` | 默认可见的真实资源 | 许可满足时允许 |
| `hidden` | 真实但默认不进入普通选择/搜索的资源 | 许可满足时允许，artifact 必须保留隐藏语义 |
| `dummy` | 占位、测试或不可选择记录 | 可以审计，但阻断整个候选文档 bundling |

生成 artifact 前必须显式移除或修正所有 `dummy`，不能静默把 dummy 变成可选择 target。

## Provenance 与许可门禁

每个 source 必须提供安全 `source_id`、名称、HTTPS 来源、获取日期和 license 状态。每个 target 至少
引用一个已声明 source；重复、未知或未使用 source 都使候选无效。

许可状态：

- `unknown`：候选可通过语义审计，但产生 `license_unknown` blocker。
- `restricted`：候选可通过语义审计，但产生 `license_restricted` blocker。
- `redistributable`：还必须提供非空 SPDX expression、HTTPS evidence URL、attribution、
  `reviewed_by` 和有效日历日期的 `reviewed_at`；缺任一字段即候选无效。

validator 不联网，也不判断网页内容或法律结论是否真实。`redistributable` 只能由实际核对授权证据的
reviewer 设置；技术门禁保证审核声明完整、可追踪，并对未知状态 fail closed。

## 只读验证

普通候选审计：

```powershell
cargo run -p hmm-games-mhw --example validate_equipment_candidates -- <candidate.json>
```

生成 artifact 前的强门禁：

```powershell
cargo run -p hmm-games-mhw --example validate_equipment_candidates -- --require-bundled <candidate.json>
```

入口只读取显式文件并输出聚合计数、固定 issue code 和数组索引 scope；不回显 resource path、名称、
URL、license 正文或其他候选值，也不写入文件。退出码：`0` 成功，`1` 语义无效，`2` bundling 被
许可/dummy 门禁阻断，`64` 用法错误，`65` JSON/版本错误，`66` 读取失败，`70` 报告序列化失败。

## 测试与后续

聚焦测试：

```powershell
cargo test -p hmm-games-mhw --test equipment_catalog_candidate --no-fail-fast
cargo clippy -p hmm-games-mhw --all-targets -- -D warnings
```

测试只使用内存人工 JSON，覆盖绝对路径、`..`、大小写碰撞、重复 stable ID、Unicode 归一化后的
重复 display name、错误 path family、dummy/hidden、许可门禁和报告脱敏。不得以真实游戏目录、
第三方 Mod、玩家数据或来源未明 catalog 作为自动测试输入。

CAT-01 完成后：AR6 可以在获得明确再分发权后扩展 armor runtime catalog；WR-01 已基于同一候选
身份/名称/许可契约完成 14 类 weapon 设计。WR-02A 可使用人工最小 catalog 实现 parser，WR-02B 的
完整 catalog 仍等待可再分发的审计输入。两者都不得绕过本门禁或把 MHW 路径规则移入 core、Tauri
或前端。
