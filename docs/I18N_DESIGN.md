# HMM 界面中英日本地化设计（I18N）

> 状态（2026-08-23）：I18N-00 设计稿，已并入维护者反馈（语言切换只进设置页；扩展性
> 前提：未来可能支持怪物猎人其他作品与更多语言）。范围：桌面端 UI 文案的 zh_cn/en/ja
> 三语本地化、语言切换与持久化，以及 catalog 名称展示的 locale 对齐路线。
> 不改动安装/存档安全链语义，不进入当前发版判断（发版前置仍只有 Sandbox Gate 全量
> catalog 复验一项，见 [WEAPON_RETARGET_DESIGN.md](WEAPON_RETARGET_DESIGN.md)）。

## 目标与非目标

目标：

- UI 全部用户可见文案支持 zh_cn / en / ja 三语，locale key 与装备 catalog 数据一致。
- 设置页「界面偏好」提供语言切换（与主题入口同区，唯一入口，不做顶栏快捷切换），
  选择持久化；默认 zh_cn（现状），首次启动可选跟随系统 locale。
- 替换目标面板的装备/武器名称按 UI locale 展示（catalog artifact 已携带三语 names）。
- 为扩展预留结构性空间：新增 UI 语言、新增游戏作品（Rise/Wilds 等，见 T16）时不需要
  重构 i18n 基建——落点是 locale 单一来源 + fallback 链 + per-game 名称 locale 能力声明，
  见「关键决策 2」。

非目标：

- 不翻译仓库文档、日志、Audit 事实、诊断导出内容——维护者/机器面向，保持稳定可 grep。
- 不改后端错误语义：Rust 侧继续只输出稳定 reason code，文案仍由前端映射层渲染。
- 不做 RTL 与完整 ICU 复数体系（中英日均无复杂复数需求，模板函数足够）。
- 安装器（NSIS/WiX）与系统对话框文案独立评估，不在本单元范围。

## 现状盘点（2026-08-23）

- 前端硬编码中文约 2462 处、分布在 108 个文件（`rg -c '\p{Han}' src/**/*.{ts,tsx}` 统计）。
- 已有集中化文案层可直接迁移：`replacementErrorText.ts`（107 行含中文）、
  `batchModLifecycleCopy.ts`（58）、`firstRunTour.ts`（116）以及各 feature 的
  `*TaskState`/`*ViewModel`/`*Copy` 文案段；其余散在组件 JSX 内。
- 后端只返回稳定错误码与事实，文案已在前端映射——架构 i18n 友好，本地化不需要动 Rust。
- catalog artifact 名称已是三语（`names.{zh_cn,en,ja}`），但 Tauri DTO 目前只投影单一
  `displayName: string`（`src/features/replacements/replacementTypes.ts:46`）。catalog
  名称按 locale 展示需要契约演进，受 GOV-03 Tauri command 契约回归门禁约束，必须独立
  切片（I18N-08），不与纯前端文案混在一个 PR。
- 生产依赖仅 5 个（react、react-dom、@floating-ui/react、lucide-react、@tauri-apps/*），
  项目一贯依赖极简。

## 关键决策 1：文案基建选型（待拍板）

**方案 A（推荐）：自研轻量 typed 字典**

- 每 feature 一个 namespace 模块：
  `const copy = { zh_cn: {...}, en: {...}, ja: {...} } satisfies Record<Locale, FeatureCopy>`；
  key 由 TS 接口锁死——任何 locale 缺 key 直接编译失败，不存在运行时 fallback 缺口。
- `useI18n()` context hook 提供当前 locale 与取词；插值用模板函数
  （如 `selectedCount: (n: number) => \`已选 ${n} 项\``），类型随函数签名走。
- 零新增依赖，符合依赖极简纪律；基建约 100 行，与现有 `*Copy.ts` 文件风格同构，
  迁移即「把中文对象补成三语对象」。
- 代价：无 ICU 生态（复数/性别/相对时间）；三语均无复杂需求，模板函数覆盖。

**方案 B：react-i18next**

- 生态成熟、支持 namespace 懒加载与 ICU；但引入 `i18next` + `react-i18next` 两个运行时
  依赖，key 默认为无类型字符串（补类型需 typegen 工具链），且与现有集中化 copy 文件
  模式重复。桌面端离线应用对懒加载收益有限。

**扩展性前提下的评估**（未来可能新增语言、新增游戏作品）：

- 新增语言在方案 A 下 = `Locale` union 加一项：核心语言由编译器强制所有 namespace 补齐；
  若某语言允许渐进翻译，声明为扩展语言（`Partial` 字典 + fallback 链兜底），不阻塞发布。
  规模到 5–10 个语言 typed 字典依然可控。
- 新增游戏在两方案下都不触碰 i18n 基建：UI 文案是 app 级 namespace，游戏侧术语
  （装备/怪物名称）永远在该游戏自己的 catalog 数据里，不进 UI 字典（见关键决策 2 的
  解耦）。
- 方案 A 的真正切换点是「开放社区翻译贡献」（非开发者提交 JSON 文件）：届时再迁移
  文件式方案，typed key 结构可机械转换，前期投入不作废。

结论：扩展性前提不改变推荐，仍为方案 A。以下切片计划按方案 A 编写，选 B 时 I18N-01
改为依赖接入与 typegen 基建，其余切片不变。

## 关键决策 2：locale 模型（含扩展性设计）

**单一来源**：`src/shared/i18n/locales.ts` 是全仓唯一 locale 定义点——
`type Locale = 'zh_cn' | 'en' | 'ja'` 加元数据表（每项含 `nativeName` 自称名、fallback
链）。新增语言只改这一个文件 + 补字典，任何组件不得自带语言判断。

**fallback 链**：核心语言（zh_cn/en/ja）字典必须全量（编译期保证）；未来扩展语言允许
`Partial` 字典，取词沿声明的 fallback 链下钻（例：`zh_tw → zh_cn → en`），链终点统一为
`en`。fallback 是基建内置行为，不是调用方职责。

**UI locale 与游戏数据 locale 解耦**（多游戏前提的核心）：

- UI 语言是 app 级偏好；装备/怪物名称语言是 per-game 数据能力。两者 key 空间一致
  （便于对齐）但互不绑定。
- 每个游戏 adapter/catalog 声明自己的可用名称 locale 集：MHW:I = {zh_cn, en, ja}；
  未来 Rise/Wilds 各自声明，可以少于或多于 UI 语言集。
- UI locale 不在某游戏名称 locale 集内时，名称展示按 fallback 链取该游戏可用的最近
  语言，UI 其余部分不受影响。杜绝「为一个游戏的数据语言限制整个应用语言」。

**其余**：默认 `zh_cn`；持久化进现有设置存储（与主题偏好同层同机制）；「跟随系统」为
一等选项（Windows 系统 locale → 映射，映射不到落 `en`）；切换即时生效（context 驱动
重渲染），不要求重启应用。

## 设置页语言切换 UI 规格（美观与人性化为验收项）

- 位置：设置页「界面偏好」区，与主题切换同区同视觉基元（复用既有控件形态，不引入
  新交互模式）；这是唯一入口，不做顶栏快捷切换。
- 语言选项永远以其自身语言显示：「简体中文 / English / 日本語」，不随当前 UI 语言翻译
  ——用户在看不懂当前语言时也能找回自己的语言。
- 「跟随系统」作为一等选项置顶，并在括注中显示当前系统解析结果（如「跟随系统（English）」）。
- 当前生效项有明确选中态；切换即时生效、无重启、全界面平滑过渡，切换后以轻量反馈
  确认（沿用现有 toast/feedback 基元）。
- 浅/深主题下均需截图验收；该控件本身是 I18N-01 试点页的一部分。

## 布局与视觉风险（硬约束）

- 英文文案通常比中文长 30–60%，日文相近或略长；现有成对断点规则会受冲击：顶栏文字
  标签 ≥1200px 显隐、`.window-tools` 1060px 隐藏阈值、状态栏两列收缩同断点等。
- 维护者验收环境为 4K + 150% 缩放（CSS 视口约 2400px），断点问题在该环境不一定复现；
  每个切片必须小步提交并由维护者截图验收，浅/深主题与 `1280x800`、`480x800` 视口至少
  各抽查一次。
- 试点页先行（Settings + About），确认文案长度与布局策略后再铺开其余 feature。

## 切片计划

| 单元 | 内容 | 出口条件 |
| --- | --- | --- |
| I18N-00 | 本设计与选型拍板 | 设计合并，方案 A/B 确定 |
| I18N-01 | 基建（Locale context、持久化、设置页语言切换）+ Settings/About 试点三语 | 试点页三语截图验收 |
| I18N-02 | mods feature 第一批（库页、工具栏、导入、分页、卡片） | 截图验收 |
| I18N-03 | mods feature 第二批（批量生命周期、外部导入、详情、重装预览） | 截图验收 |
| I18N-04 | profiles + install-recovery | 截图验收 |
| I18N-05 | dashboard + game-setup + game-launch | 截图验收 |
| I18N-06 | categories + diagnostics + settings 余量 + shell/frame/onboarding 短文案 | 截图验收 |
| I18N-07 | 错误码映射层（`replacementErrorText` 等）+ 长文案（`firstRunTour`） | 三语 key 完备编译断言 |
| I18N-08 | catalog 名称 locale 对齐：DTO 契约演进（单 `displayName` → 按 locale 投影或多语携带，二选一在该切片内定，形态必须兼容 per-game 名称 locale 能力声明与 fallback 链）+ 跨语言检索语义确认 | 契约门禁通过 + 面板截图 |

每切片独立 PR、独立回滚。I18N-01~07 为纯前端，聚焦检查（typecheck/lint/test + 截图）；
I18N-08 触及 Tauri 契约，验证升级为完整 `scripts/verify.ps1`。

## 验收与门禁

- 每切片：`pnpm typecheck` / `lint` / `test` 全绿 + 受影响页面浅/深主题截图。
- key 完备性由类型系统保证（方案 A：缺任一 locale 的 key 编译失败）。
- 禁止把 reason code、Audit 事实、日志字符串、`stable_id` 类标识纳入翻译。
- 与 Sandbox Gate 复验、CLI-3B 并行推进，优先级建议 P2，不进入当前发版判断。
