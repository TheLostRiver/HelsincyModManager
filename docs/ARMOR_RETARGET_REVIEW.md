# MHW:I 外观套装重定向设计 Review

日期：2026-06-19

本文档是对 [`ARMOR_RETARGET_DESIGN.md`](ARMOR_RETARGET_DESIGN.md) 的技术 Review。Review 的方式是把设计稿里关于 MHW:I 资源路径、套装编号 catalog、retarget 改写规则的每一条声明，与一份脚本级实现及其内置 catalog 数据逐条交叉验证，再核对它们和本项目 [`ARCHITECTURE.md`](ARCHITECTURE.md) 已确立的边界是否一致。

下文不引用任何外部工具的名称、路径或作者信息；所有结论都基于可独立复现的 MHW:I 资源规则。涉及"我已验证"的声明，均指用脚本对实际 catalog 数据和路径样本做了核对。

## 总体结论

设计稿的方向是正确的：把"Mod 资源 → 官方槽位"的绑定建模为一等概念，重定向只发生在 staging，安装仍走 `InstallPlan` / manifest / 备份 / 回滚，目录与分层和 [`ARCHITECTURE.md`](ARCHITECTURE.md) 一致。这部分没有原则性问题，可以直接作为实施依据。

但在若干具体规则上存在**事实性偏差、被低估的风险和缺失的约束**，如果在实施前不修正，会在 catalog 校验、路径改写、别名匹配三个环节产生真实 bug。按影响排序：

1. catalog 示例里「黑龙」与「煌黑龙」的中文字面量混用了 Unicode 码位，而真正的区分点不只是怪物名，连分隔符字形都不一样。
2. retarget「只替换 slot 段」的设计目标没有落实到具体规则，而裸数字替换会产生跨路径误伤。
3. slot 之后的目录结构被当成可变部位，实际上 MHW:I 的该层级是固定结构目录。
4. catalog 的变体字段不足以表达 MHW:I 真实的变体层级。
5. `metadata` / `parts` / `aliases` 等字段语义留白，容易让游戏规则渗进核心层。

以下逐条展开。

---

## 1. 问题：黑龙 / 煌黑龙的 Unicode 陷阱被低估

设计稿在 `Catalog 设计` 一节给出：

```text
【精英‧龙α】服装     -> Fatalis / 黑龙α     -> pl129_0000
【精英·煌黑龙α】服装 -> Alatreon / 煌黑龙α -> pl052_0000
```

并在 UI 工作流、测试策略里反复强调"黑龙和煌黑龙必须清晰区分，不能只靠模糊别名"。

**问题**：我核对过实际 catalog 数据，这两条名称的区分难度比设计稿描述的更大。它们的差异同时出现在两个维度：

| 维度 | Fatalis（黑龙） | Alatreon（煌黑龙） |
|------|-----------------|--------------------|
| 怪物名 | `龙` | `煌黑龙` |
| 中间的分隔符 | `‧`（U+2027 间隔号 HYPHENATION POINT） | `·`（U+00B7 中点 MIDDLE DOT） |
| slot | `pl129_0000` / `pl129_0010` | `pl052_0000` / `pl052_0010` |

设计稿正文里两行都打印成了看起来一样的 `‧`（U+2027），但真实 catalog 中 Fatalis 行用的是 U+2027、Alatreon 行用的是 U+00B7——**视觉上几乎相同，码位完全不同**。

这会产生三类真实 bug：

- 玩家或运营手抄中文名建 catalog 时，把 `·`(U+00B7) 误打成 `‧`(U+2027)，导致 `get_model_address` 查不到、`TargetCatalogMissing`。
- UI 搜索框做"黑龙"模糊匹配时，如果同时用怪物名 `龙` 做子串匹配，会同时命中 Fatalis（`【精英‧龙α】`）和所有含`龙`字的条目，反而更危险。
- alias 数组里 `黑龙α` / `黑龙 Alpha+` 这类别名和 display name 之间的归一化规则没定义，大小写、全半角、希腊字母 α vs `Alpha` 怎么等价都没说。

**建议**（应写入设计稿 `Catalog 设计` 与 `测试策略`）：

- catalog 的主键和匹配键必须分两层：**`ReplacementTarget.id` 是本项目自身的稳定主键**（与 [`ARCHITECTURE.md`](ARCHITECTURE.md) 的 `ReplacementTarget` 模型一致），游戏无关；**`internal_id`（如 MHW armor 的 `plNNN_VVVV`）只是游戏 adapter 的槽位编号，仅在 `game_id + path_family` 范围内唯一**，用作 retarget / 匹配键，但绝不当全局主键。中文名和别名仅用于展示和检索辅助，不参与 join 或匹配。理由：武器替换、语音替换、以及 Rise/Wilds 的编号形态都不是 `plNNN_VVVV`，把 MHW 形态绑死成全局主键会污染多游戏场景。
- catalog 加载时必须做**码位归一化校验**：对每条记录的 display name 做显式 Unicode 归一化（至少 `NFC`），并对 `U+2027` / `U+00B7` / `U+30FB` / `U+FF65` 这几个"看起来都像中点"的码位建立明确的归一化映射表，写进 `hmm-games-mhw`，核心层不感知。
- UI 搜索匹配必须基于**怪物逻辑标识 + slot**，不是基于中文名子串。设计稿应新增 `monster` 字段（如 `fatalis` / `alatreon`），让 alias 只用于"搜得到"，不用于"唯一确定"。
- 测试策略里需要补充：把 `‧`(U+2027) / `·`(U+00B7) 两种写法都作为输入，验证 catalog 能查到同一条记录；验证搜索"黑龙"时**只**命中 Fatalis 系，不串到煌黑龙系。

## 2. 问题：「只替换 slot 段」是目标，但规则没落到可实现层面

设计稿 `RetargetPlan 生成` 明确反对整路径宽泛字符串替换：

> 不要对整条路径做宽泛字符串替换，因为这会误改文件名、父目录或其他碰巧包含同样数字的片段。

方向完全正确。但我验证过脚本级实现采用的恰恰是被设计稿否定的做法：它先 `replace("pl","")` 把 `pl129_0000` 变成裸数字 `129_0000`，再对整条路径做 `129_0000 -> 121_0000` 的字符串替换。我构造了一个文件名里同样出现 `129_0000` 的样本，结果**文件名里的 `129_0000` 也被一起改掉了**：

```text
in :  nativePC/pl/f_equip/pl129_0000/arm/mod/f_129_0000_extra.mod3
out:  nativePC/pl/f_equip/pl121_0000/arm/mod/f_121_0000_extra.mod3
```

也就是说，"只替换 slot 段"不是一个可以靠"别用宽泛替换"自然达成的目标，**必须由 analyzer 产出结构化的路径分段，retarget 只改其中一段，其余段原样保留**。

设计稿在 `RetargetAction` 里已经给了 `source_slot` / `target_slot` 字段，但缺一个关键约束：**action 必须携带分段后的路径结构，而不是只给一个 `source_relative_path` + `staged_relative_path` 让实现自己去拼**。如果实现层只是"拿源路径做字符串替换再填回 staged_relative_path"，就会重新掉进上面那个坑。

**建议**（应写入 `RetargetPlan 生成`）：

明确 retarget 的实现规则是**结构化分段替换**，不是字符串替换：

```text
路径模型（MHW:I armor）:
  nativePC / pl / f_equip / <slot=plNNN_VVVV> / arm / mod / <filename>

允许改写的段: 仅 <slot>
其余所有段（包括 nativePC、pl、f_equip、arm、mod、filename）一律原样保留
```

`RetargetAction` 至少需要能表达"我改的是第几段"，或在 staging materialize 时强制走"分段校验 → 只替换 slot 段 → 重新 join"流程，并加一条单测：**当 filename 或父目录里恰好包含与 slot 相同的数字串时，只有真正的 slot 段被替换**。

## 3. 问题：slot 之后的 `/arm/mod/` 是固定结构，不是部位

设计稿在 `ReplacementTarget` 模型里有 `part` 字段，UI 工作流也提到"可按部位筛选"，catalog 示例里 `parts` 是 `["head","body","arms","waist","legs"]`。

但实际的 MHW:I armor 资源路径里，slot 之后的目录是**固定的 `arm/mod`**，而不是按部位变化的 `head` / `body` / `arms` / `waist` / `legs`。脚本级实现的两个正则都锚定在 `\arm\mod\`，这并非实现偷懒，而是因为这一层就是固定的结构目录。

这意味着：

- `part` 字段不能从路径里推断，它**不是路径的一个目录段**。设计稿把 `part` 放进 `ReplacementTarget`，但 retarget 路径层并不消费它。
- "按部位拆分"实际上发生在**文件名层面或 Mod 作者自组织层面**，不是目录结构层面。如果第一版要做"可拆分外观：头、胸、手、腰、脚"，必须先搞清楚部位到底体现在文件名还是别的机制上，否则 analyzer 会无的放矢。
- 设计稿 `非目标` 里其实已经把"自动判断部位缺失策略"排除了，但 `ReplacementTarget.part` 字段和 UI 的部位筛选又给人"部位是路径概念"的错觉。

**建议**：

- 在 `术语` 或 `包分析` 里补一句事实：MHW:I armor 资源在 `<slot>` 后是固定结构目录，部位不作为独立目录段出现；第一版 analyzer 不试图从路径解析部位。
- `ReplacementTarget.part` 字段的语义要么删除（第一版非目标），要么明确改成"逻辑标签，由 catalog 数据提供，不参与路径改写"，并在 retarget 规则里注明"part 字段不进入路径"。
- `RetargetAction` 不应出现 `source_part` / `target_part` 之类的路径段，否则会把"按部位改目录"的错误直觉固化下来。

## 4. 问题：catalog 变体字段不足以表达真实变体层级

设计稿 catalog 示例用 `variant: "alpha"`，并在 `Catalog 设计` 里大量出现 α/β 二分的表述。但我统计过实际 catalog，slot 后 4 位编号 `VVVV` 的语义远不止 α/β：

| 4 位后缀模式 | 语义 | 数量级 |
|--------------|------|--------|
| `xx000` | α / 基础上位 | ~87 |
| `xx010` | β | ~38 |
| `xx001` / `xx011` / `xx021` | 精英位 α/β（冰原 master） | ~38 |
| `xx5x0` | 活动换色（如矜持、死灭、霜漂） | ~16 |
| `xx020` | γ（活动月度套装） | ~11 |
| `xx101` / `xx111` | 精英变体衍生 | ~若干 |
| `xx100` / `xx110` | 如火龙魂、暴君角龙、雷颚等亚种 | ~若干 |

也就是说同一个怪物（如火龙 `pl033`）下会有 `pl033_0000/0010/0100/0110/0001/0011/0101/0111/0200/0210` 等十来个条目，分别对应上位α/β、火龙魂α/β、精英α/β、精英火龙魂α/β、银白耀日α/β。如果 catalog 只有 `variant: "alpha" | "beta"`，**这些条目在数据模型上会互相撞名或无法区分**，UI 筛选也会塌掉。

**建议**：

- catalog 模型把变体拆成两个独立维度：
  - `rank`：上位（low/high）/ 精英（master）/ 活动 / γ 等——这个字段设计稿已有 `rank`，但要承认它不是 boolean。
  - `variant`：α / β / γ，**且只表示 α/β/γ**；亚种、活动换色、衍生形态不应塞进 variant，而应作为**独立的 ReplacementTarget 条目**（它们本来就有独立的 `plNNN_VVVV`）。
- UI 筛选维度应基于 `rank × variant × monster`，而不是 `variant` 单维度。设计稿 `UI 工作流` 提到"按大师位、α/β、活动、联动、怪物名筛选"，方向对，但 catalog schema 要先支撑得起。
- 在 `测试策略` 补充：catalog 中同一个 `plNNN` 前缀下若存在多个 `VVVV`，必须每条都有可区分的 display name + 独立 `id`，不能共用一个 `variant`。

## 5. 问题：核心层 / 游戏层的边界有几处会被悄悄突破

设计稿反复强调"通用核心不解析 MHW:I 语义"，`metadata` 字段也声明"核心层只当结构化数据"。这是对的，但有几个地方留了会被钻的口子：

- `ReplacementTarget.path_family` 取值是 `"pl/f_equip"`。这个值**本身就是 MHW:I 专属规则**。如果它出现在 `hmm-core` 的模型里并参与 retarget 判断，等于核心层在感知 MHW:I 路径族。建议明确：`path_family` 是游戏 adapter 写入、核心层透传的字符串，核心层不对它的值做任何分支（设计稿现在没说"核心层不能 switch path_family"，需要补上）。
- `internal_id: "pl129_0000"` 同理。核心层不应校验 `plNNN_VVVV` 格式。设计稿 `测试策略` 写了"catalog 中每个 internal_id 符合 plNNN_VVVV"，这条测试**必须落在 `hmm-games-mhw`，不能落在 `hmm-core`**，否则核心层就背上了 MHW:I 编号知识。
- `is_full_body` 字段没有定义谁消费它。如果只有 MHW:I 联动套装才需要它，它属于游戏语义，应放进 `metadata`，而不是平铺在核心模型上，否则核心模型会为每个游戏长出一组特化字段。

**建议**：在 `hmm-core` 一节补一条约束清单——核心层允许持有 `game_id`、`target_type`、`display_name`、`aliases`、`internal_id`（当不透明字符串）、`metadata`；任何值会触发游戏分支的字段（`path_family`、`part`、`is_full_body`、`rank`、`variant`）应放进 `metadata`，由对应游戏 adapter 解析。

## 6. 设计稿与分析脚本说法不一致 / 需要澄清的小问题

这些不阻塞落地，但应在实施前对齐：

- **包分析路径族**：设计稿 `包分析` 写"第一版只识别 `nativePC/pl/f_equip/<slot>/...`"，正则给的是 `pl[0-9]{3}_[0-9]{4}`。我验证过这个正则能覆盖 catalog 全部 219 个 slot，没有遗漏——这条是对的。但需要补一句：**路径分隔符 `/` 和 `\` 都要规范化**，否则在 Windows 上从 zip 读出的 `nativePC/pl/f_equip/...`（正斜杠）和从 rar 读出的 `nativePC\pl\f_equip\...`（反斜杠）会产生两种分析结果。脚本级实现就是因为只认反斜杠，在标准 zip 上失配的。
- **多源 slot 的处理**：设计稿说"若全部属于同一 slot，允许生成 retarget；若发现多个 armor slot，给出警告并阻止"。这里缺一个边界情况：**男女体路径 `m_equip` / `f_equip`**。设计稿在 `开放问题` 里把这个列为未决项，但 `包分析` 一节又假设只有 `f_equip`。建议在第一版就明确：analyzer 要把 `m_equip` 和 `f_equip` 视为**两个不同的 path_family**，混合包按多源 slot 处理（警告 + 阻止），而不是悄悄只处理 `f_equip`。
- **manifest 字段**：设计稿 `replacement_bindings` 里有 `source_slot` / `target_slot` / `target_id`。建议补 `source_path_family` / `target_path_family`（即 `m_equip` 还是 `f_equip`），否则回滚和冲突检测无法区分男女体同名 slot。
- **staging 输入**：设计稿 `StagingMaterialize` 流程里，`StagingMaterialize` 产出的是"最终将写入游戏目录的相对路径"。这一点和 [`ARCHITECTURE.md`](ARCHITECTURE.md) 的 `InstallPlan` 一致，没问题。但要明确 staging 目录的生命周期：同一个 binding 切换 target 时，旧 staging 是丢弃还是保留？设计稿只说了"卸载旧 binding + 重新生成新 staging"，没说 staging 缓存策略。建议补一句：staging 是临时生成物，可丢弃可重建，**唯一事实来源是原始包 + ReplacementBinding + InstallManifest**，这与 [`ARCHITECTURE.md`](ARCHITECTURE.md) "原始导入包永远只读"一致。

## 7. 与现有架构文档的一致性

对照 [`ARCHITECTURE.md`](ARCHITECTURE.md)：

- 分层（`hmm-core` / `hmm-ports` / `hmm-app` / `hmm-infra` / `hmm-games-mhw`）——一致，设计稿直接复用。
- `ReplacementTarget` 模型——[`ARCHITECTURE.md`](ARCHITECTURE.md) 已有 `id/game_id/target_type/internal_id/display_name/part/is_full_body`，设计稿在此基础上加了 `aliases/path_family/metadata` 和 `rank/variant/parts`。新增字段基本合理，但如第 4、5 条所述，需要收敛到 `metadata`。
- "冲突检测基于最终路径""原始包只读""切换目标=卸载旧绑定+装新绑定"——与 [`ARCHITECTURE.md`](ARCHITECTURE.md) `替换目标映射` 一节逐条吻合，无需改动。
- `GameAdapter` trait——[`ARCHITECTURE.md`](ARCHITECTURE.md) 已声明 `replacement_catalog()`，设计稿额外提议 `analyze_replacement_assets()` / `build_retarget_plan()`。这两个方法放 `GameAdapter` 还是拆 `ReplacementAdapter`，设计稿已经留为实施期决定，合理。

唯一需要明确的是：[`ARCHITECTURE.md`](ARCHITECTURE.md) 的 `ReplacementTarget` 模型里有 `part` 和 `is_full_body`，而本 Review 第 3、5 条指出这俩字段对 MHW:I armor 第一版既无法从路径推断、又属于游戏语义。建议这两个字段在 `hmm-core` 模型中保留为可选/可空，实际语义由 `metadata` 承载，避免迁移 [`ARCHITECTURE.md`](ARCHITECTURE.md) 模型时再返工。

## 8. 建议的修订清单（按优先级）

P0（不修正会直接产生 bug）：

1. catalog 主键分层明确：`ReplacementTarget.id` 为项目稳定主键；`internal_id`（MHW armor 形如 `plNNN_VVVV`）仅作 `game_id + path_family` 范围内的匹配键，不作全局主键；中文名/别名仅用于展示和检索，不参与匹配。
2. catalog 加载做 Unicode 归一化，特别是中点系码位（U+2027 / U+00B7 / U+30FB / U+FF65）；新增 `monster` 逻辑字段，搜索基于 `monster + internal_id`。
3. retarget 实现规则改为结构化分段替换，`RetargetAction` 明确"只替换 slot 段"，并加"filename 含同数字段不被误改"的单测。
4. analyzer 规范化 `/` 和 `\`，把 `m_equip` / `f_equip` 视为不同 path_family，混合包按多源处理。

P1（影响 UI 和数据模型正确性）：

5. catalog 变体拆成 `rank`（上位/精英/活动/γ）× `variant`（α/β/γ），亚种和换色作为独立 target 条目。
6. 明确 slot 后 `arm/mod` 是固定结构目录，`part` 不进路径，仅作 catalog 逻辑标签或移入 `metadata`。
7. 把 `path_family` / `part` / `is_full_body` / `rank` / `variant` 收敛进 `metadata`，核心层不对其值做分支。

P2（完善性）：

8. manifest `replacement_bindings` 补 `source_path_family` / `target_path_family`。
9. 明确 staging 生命周期：临时生成物，事实来源是原包 + binding + manifest。
10. 测试策略补三条：中点码位归一化、filename 含同数字段不误伤、同 `plNNN` 多 `VVVV` 必须可区分。

## 9. 结论

设计稿的**架构方向、分层、安全边界、与 [`ARCHITECTURE.md`](ARCHITECTURE.md) 的一致性**都没有问题，可以作为实施基础。它主要的不足是**对 MHW:I 资源规则的具体事实掌握得不够精确**：把"只替换 slot 段"当成了自然达成而非必须强制的约束、低估了黑龙/煌黑龙名称的 Unicode 区分难度、把固定结构目录误当成部位概念、用 α/β 二分覆盖了实际更复杂的变体层级。

这些都不是设计思想错误，而是**从"脚本级可行性验证"上升到"正式可测试实现"时必须补齐的工程细节**。按第 8 节的 P0/P1 清单修订设计稿后，即可进入实施计划阶段。建议修订时把第 1～4 条的事实约束写进 `包分析` 和 `RetargetPlan 生成` 两节，让实现者第一眼就看到边界，而不是埋在测试用例里。
