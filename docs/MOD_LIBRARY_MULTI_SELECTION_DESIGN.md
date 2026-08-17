# Mod 库多选与批量操作交互设计

> 状态：设计草案，尚未实现
>
> 日期：2026-08-18
>
> 范围：Mod 管理页的选择交互、批量模式、跨页选择与既有批量生命周期工作流接入

## 文档定位

本文定义 Mod 管理页如何让用户可发现、可预测地选择多个 Mod，并把选择结果交给现有批量安装、
真正重装、卸载、结果和重试工作流。

本文不重新定义批量写入语义。以下文档仍是对应边界的权威来源：

- [批量 Mod 生命周期领域设计](BATCH_MOD_LIFECYCLE_DESIGN.md)：BatchPlan、preview、seal、apply、
  partial result、retry、锁和恢复语义。
- [Mod 库分页设计](MOD_LIBRARY_PAGINATION_DESIGN.md)：服务端分页、查询上下文和页面缓存边界。
- [前后端契约](FRONTEND_BACKEND_CONTRACT.md)：Tauri command、DTO、稳定错误码和 Sandbox 门禁。
- [架构设计](ARCHITECTURE.md)：React、Tauri、应用层和基础设施层职责。

本文中的“批量模式”是前端选择状态，不是写入授权。用户选中多个 Mod 不会绕过 preview、确认、
plan token、写入 admission、同 game/profile 串行、manifest、backup、rollback 或 recovery。

## 背景与现状

当前 Mod 管理页已经具备大部分批量基础：

- `selectedIds` 保存跨页选择的 Mod ID。
- `modSelection.ts` 已支持 `replace` 和 `toggle` 两种纯选择操作。
- “选择本页”和“反选本页”可以产生多选集合。
- 多于一项时，安装、真正重装和卸载已经分派到批量 workflow。
- 批量 preview、确认、执行、结果、部分失败和 retry 面板已经接入。
- 后端每批最多接受 100 个 item。

当前主要缺口不是批量 runner，而是选择体验：

- 卡片普通点击固定使用 `replace`，用户无法逐项构造多选。
- “选择本页/反选本页”直接暴露在普通快捷操作中，但没有明确的批量模式。
- 单项与批量行为只由 `selectedIds.size` 推导，取消到 0 或 1 项时容易产生隐式模式切换。
- 不同视图的选中指示不统一，机能视图甚至隐藏了选中圆圈。
- 用户不知道当前选择是否跨页，也看不到 100 项上限。
- Production 安装态的批量 Tauri 写入仍未开放，前端不能把 Sandbox 能力展示成可发布能力。

## 目标

1. 在“快捷操作”区域提供明确、可发现的“批量选择”入口。
2. 保持普通点击的单选习惯，同时支持 `Ctrl + 点击`快速进入并追加多选。
3. 让批量模式在 0、1 或多项选择下都保持稳定，不根据数量隐式退出。
4. 让跨页选择、清空时机和 100 项上限可见、可预测。
5. 让批量操作始终经过现有 preview、确认、结果和 retry 闭环。
6. 在四种卡片视图、键盘操作、屏幕阅读器和窄窗口下保持一致行为。
7. 明确 Sandbox 与 Production capability 边界，避免用户进入必然失败的写入路径。

## 非目标

- 不在本切片开放 Production 批量写入。
- 不修改 BatchPlan、token、journal、retry 或单项生命周期领域语义。
- 不允许前端循环调用单项 command 来模拟批量操作。
- 不实现“选择全部匹配结果”。首版只支持当前已加载页面上的明确选择。
- 不实现 `Shift + 点击`连续范围选择、框选、拖拽选择或长按选择。
- 不跨应用重启持久化选择，也不在离开 Mod 管理页后保留选择。
- 不让前端根据卡片缓存自行决定某项最终可安装、可重装或可卸载。
- 不新增文件系统路径、manifest 正文、backup ref 或其他敏感事实到前端状态。

## 术语

| 术语 | 含义 |
| --- | --- |
| 普通模式 | 默认选择模式，卡片点击保持单选行为，展示单项生命周期操作 |
| 批量模式 | 显式进入的多选状态，卡片点击切换选中，展示批量生命周期操作 |
| 选择集合 | 当前页面持有的 `Set<ModId>`，可包含其他分页中的 Mod ID |
| 本页选择 | 选择集合与当前服务端分页结果的交集 |
| 查询上下文 | 当前 game/profile、搜索文本、筛选、排序和其他决定结果集的条件 |
| 选择意图 | 卡片传给页面的纯交互意图，不包含 React 事件对象 |
| 批量能力 | 后端明确投影的批量 preview/apply capability，不由前端环境猜测 |

## 总体状态模型

选择模式与选择数量必须分开保存：

```ts
type ModSelectionMode = "single" | "batch";

type ModLibrarySelectionState = {
  mode: ModSelectionMode;
  selectedIds: ReadonlySet<string>;
};
```

不能使用 `selectedIds.size > 1` 推导批量模式。否则会出现以下问题：

- 批量模式取消到 1 项时突然切回单项操作。
- 取消最后一项时批量工具栏消失，用户无法继续选择。
- `Ctrl + 点击`进入批量模式后，松开 Ctrl 会失去明确模式。
- 关闭 preview 后，界面可能因选择数量变化切换成另一套操作语义。

批量 workflow 状态与选择状态正交：

```text
selection: single | batch
workflow:  idle | resolving | preview | starting | running | result
```

进入 `starting` 或 `running` 后冻结选择；终态如何清理选择由结果规则决定。

## 选择状态机

| 当前模式 | 用户事件 | 结果 |
| --- | --- | --- |
| 普通 | 点击未选卡片 | 清空旧选择，只选目标卡片 |
| 普通 | 点击唯一已选卡片 | 取消该卡片，保持普通模式 |
| 普通 | 点击“批量选择” | 进入批量模式，保留当前 0 或 1 项选择 |
| 普通 | `Ctrl + 点击`卡片 | 进入批量模式，并 toggle 目标卡片 |
| 普通 | `Ctrl + Space`聚焦卡片 | 进入批量模式，并 toggle 目标卡片 |
| 批量 | 点击卡片 | toggle 目标卡片 |
| 批量 | `Ctrl + 点击`卡片 | toggle 目标卡片，与普通点击结果一致 |
| 批量 | `Space`或 `Ctrl + Space` | toggle 聚焦卡片 |
| 批量 | 取消最后一项 | 保持批量模式，选择数变为 0 |
| 批量 | 点击“退出批量选择” | 清空选择、重置未执行 preview、回到普通模式 |
| 批量 | 按 `Escape` | 没有上层对话框时退出并清空；有对话框时先关闭最上层对话框 |

### 首次 Ctrl 点击

普通模式下首次 `Ctrl + 点击`必须按以下顺序处理：

1. 保留当前单选集合。
2. 把模式切换为 `batch`。
3. 对目标卡片执行一次 `toggle`。
4. 通过 `aria-live`宣布新的已选数量。

如果当前唯一选中的卡片就是目标卡片，首次 `Ctrl + 点击`会取消它，结果是“批量模式，已选 0 项”。
该结果是有意设计，不能因为选择为空自动退出批量模式。

### 松开 Ctrl

Ctrl 只是进入批量模式的快捷方式，不是临时修饰状态。松开 Ctrl 后：

- 批量模式继续保持。
- 普通点击仍执行 toggle。
- 用户必须显式退出批量模式，或在成功结果收尾时由状态机退出。

## 选择意图边界

卡片组件可以读取 pointer/keyboard event 来识别 Ctrl，但不能把 React 事件对象传入纯选择逻辑。

推荐的组件边界：

```ts
type ModCardSelectionIntent =
  | { kind: "primary"; modId: string }
  | { kind: "toggle"; modId: string; source: "ctrl-pointer" | "ctrl-keyboard" };
```

页面收到意图后，再结合当前 `selection.mode` 选择 `replace` 或 `toggle`。`applyModSelection` 只处理
集合运算，不知道 Ctrl、鼠标、键盘、React、卡片视图或批量 workflow。

禁止以下模式：

```ts
// 不允许：纯状态逻辑依赖 React 事件。
applyModSelection(previous, modId, event);
```

## 快捷操作区

### 显式入口

“快捷操作”标题栏增加带 `ListChecks` 图标和文字的模式按钮：

| 状态 | 文案 | 行为 |
| --- | --- | --- |
| 普通模式 | 批量选择 | 进入批量模式，保留当前单选 |
| 批量模式 | 退出批量选择 | 清空选择并退出 |

按钮必须使用 `aria-pressed` 表示当前是否处于批量模式。图标和文字同时显示，不能只依赖用户猜测图标。

### 普通模式

普通模式显示：

- 导入 Mod、第三方迁移和导入新版本。
- 刷新。
- 当前单项可用的预览、安装、真正重装和卸载操作。
- “批量选择”入口。

普通模式隐藏：

- 选择本页。
- 反选本页。
- 清空批量选择。
- 批量安装、批量重装和批量卸载。

### 批量模式

批量模式显示：

- `已选 X / 100`。
- `本页已选 Y / 本页 N 项`。
- 选择本页。
- 反选本页。
- 清空选择图标按钮，tooltip 和 accessible name 均为“清空选择”。
- 批量安装、批量真正重装和批量卸载。
- 退出批量选择。

导入 Mod、第三方迁移和导入新版本不属于当前选择批次。为避免工具栏过载，批量模式可以保留导入入口，
但必须在视觉上与选择命令、生命周期命令分组。

### 操作文案

批量模式中的操作必须明确写“批量”：

```text
批量安装
批量重装
批量卸载
```

即使当前只选中 1 项，也继续使用批量文案和 batch workflow。模式决定安全协议，不能因为数量从 2 变为
1 就退回单项 command。

## 卡片交互与视觉

### 固定选择指示器

经典、增强网格、紧凑列表和机能面板四种视图都使用同一个固定位置的选择指示器槽位：

- 槽位尺寸和定位稳定，进入批量模式时不能推动标题、封面或状态徽标。
- 普通模式可以隐藏指示器视觉，但保留布局槽位。
- 批量模式显示未选/已选两态，不能只靠卡片边框或颜色表达。
- 已选状态可以叠加卡片边框或背景，但文字、图标和状态徽标不能在 hover 时消失。
- 机能面板视图不得继续隐藏选择指示器。

指示器是卡片语义的一部分，不在可点击卡片内部嵌套第二个交互按钮，避免无效的嵌套交互结构。

### ARIA 语义

普通模式：

- 卡片使用 `role="button"`。
- 使用 `aria-pressed` 表示当前单选状态。
- `Enter` 或 `Space`执行普通单选。

批量模式：

- 卡片使用 `role="checkbox"`。
- 使用 `aria-checked` 表示是否选中。
- `Space`、`Enter`和点击都执行 toggle。
- `aria-label`包含 Mod 名称和当前选择状态。

选择计数使用 `role="status"`、`aria-live="polite"`和 `aria-atomic="true"`。达到上限、清空选择或因
查询上下文变化清空时，都需要提供一次简短、稳定的通知。

### 右键菜单

普通模式保持现有行为：右键未选卡片时，将其变成唯一选择并打开单项菜单。

批量模式下右键不得隐式重写多选集合：

- 打开菜单时保留当前选择。
- “查看详情”等只读单项操作可以针对右键目标。
- 单项安装、重装、卸载等写操作禁用，并提示“批量选择中，请使用上方批量操作”。
- 不允许右键一个未选卡片后静默把整个批次替换成该卡片。

## 100 项上限

前端上限是用户反馈和减少无效请求的第一层；后端 `BatchResourceLimits` 仍是权威校验。

### 单卡片 toggle

- 当前少于 100 项时，可以新增。
- 当前恰好 100 项时，新增选择不改变状态，并提示“每批最多选择 100 个 Mod，取消一项后可继续添加”。
- 已选卡片始终可以取消，不受上限阻断。

### 选择本页

“选择本页”必须原子计算结果：

- 如果加入本页全部未选项后不超过 100，整体应用。
- 如果会超过 100，不做部分选择，并显示还差多少可用名额。
- 不按当前渲染顺序静默截断到 100。

### 反选本页

先计算完整反选结果：

- 结果不超过 100 时整体应用。
- 结果超过 100 时整体拒绝，原选择不变。
- 取消已有项的能力不能因为上限而丢失，用户仍可逐项取消或使用“清空选择”。

## 跨页选择与清空规则

### 保留选择

以下操作不改变查询语义，保留选择：

- 翻到上一页或下一页。
- 直接跳页。
- 修改每页数量。
- 切换经典、网格、列表或机能视图。
- 显示或隐藏分类标签。

选择计数必须展示全局选择数和本页选择数，避免用户误以为只选中了当前页面。

### 清空选择

以下操作改变结果集或底层事实，清空选择并重置未执行的 batch preview：

- 修改搜索文本。
- 修改筛选条件。
- 切换活动配置档或配置档变为不可用。
- 用户主动刷新 Mod 库。
- 单项写入完成后刷新 durable facts；批量全部成功并关闭结果后再清空批量选择。
- 离开 Mod 管理页。

如果清空前选择不为空，显示一次低干扰通知，例如：

```text
筛选条件已变化，已清空 12 项批量选择。
```

搜索输入连续变化时，第一次清空后选择已为空，不重复发送通知。

### 未来排序

如果后续增加排序：

- 仅改变展示顺序且保持同一结果集时可以保留选择。
- 如果排序同时改变查询 snapshot 或服务端 selection identity，必须清空。

## “选择本页”不是“选择全部”

首版只允许对已经返回的页面项进行选择。文案必须使用“选择本页”，不能写“全选”。

真正的“选择全部匹配结果”需要后端签发绑定以下事实的 selection token：

- game/profile。
- 搜索、筛选、排序和结果 snapshot。
- 总数量和 100 项资源上限。
- 过期时间和执行时重验规则。

在该能力实现前，前端不能遍历分页、猜测全部 ID 或把当前页冒充全部匹配结果。

## 生命周期操作分派

操作分派由选择模式决定，不只看选择数量：

| 模式 | 选择数 | 操作路径 |
| --- | ---: | --- |
| 普通 | 0 | 生命周期操作禁用 |
| 普通 | 1 | 现有单项 preview/install/reinstall/uninstall |
| 批量 | 0 | 批量生命周期操作禁用，保留选择命令 |
| 批量 | 1..100 | 现有 batch prepare/preview/seal/start/result/retry |

批量模式不能先按卡片缓存静默删掉“不适用”项。混合选择交给后端 preview 返回：

- ready item 数。
- blocked item 数。
- global blocker。
- item blocker 的稳定 reason code。
- warning 和执行策略。

前端可以根据已知状态提供友好提示，但后端 preview 是最终判断。尤其不能只读取当前页卡片状态来推断
跨页选择是否可执行。

## Preview、确认、结果与 Retry

### Preview 前

1. 捕获当前 `selectedIds`、active game/profile、operation 和 execution policy。
2. 对选择 ID 做确定性排序或交给既有 batch request 规范化。
3. 解析 exact revision、installed manifest 和 replacement target facts。
4. 调用既有 batch preview，不执行写入。

从开始解析到 preview 关闭期间，选择集合保持稳定。若查询上下文变化，当前请求作废并关闭 preview。

### Preview 面板

面板至少展示：

- 操作类型和总项数。
- ready、blocked 和 warning 数量。
- global blocker 与按稳定 code 映射的用户文案。
- 默认 `stop_on_failure` 和显式可选的 `continue_on_item_failure`。
- 需要 replacement target 选择的 Mod 数量。

`continue_on_item_failure` 只表示继续处理可隔离失败，不能越过 global blocker、recovery required、
写入 admission、游戏运行状态或未知玩家文件事实。

### 关闭 Preview

用户在没有执行时关闭 preview：

- 保留批量模式。
- 保留原选择集合。
- 丢弃 preview token 和临时 UI 状态。
- 下一次操作重新读取 facts 并生成 preview。

### 确认和执行

用户确认后：

- 冻结卡片选择和查询控件。
- 使用后端签发的 opaque token 启动 batch。
- 进度事件必须携带 task id、batch id、attempt number 和稳定 phase。
- UI 不允许在运行中切换 profile、刷新或启动另一批同 scope 写入。

### 成功结果

全部成功后关闭结果面板：

- 清空选择。
- 退出批量模式。
- 刷新 Mod 库 durable facts。
- 返回普通模式，不自动选中某个结果项。

### 部分失败或取消

部分失败、取消或存在 recovery/evidence degradation 时：

- 不静默清空结果。
- 结果面板展示 succeeded、blocked、failed、recovery required、cancelled 和 skipped 数量。
- retry 只使用后端从 sealed batch 派生的 `retryable` 集合，前端不能提交任意 Mod ID。
- 关闭结果前明确提示未完成项和是否还能 retry。
- 用户显式关闭后回到批量模式并保留原选择，用于检查或重新发起操作；旧 token 和旧 preview
  立即丢弃，下一次操作必须重新读取 facts。

## Capability 与运行环境

### 当前边界

现有 Tauri 批量生命周期命令只在设置了 `HMM_SANDBOX_DATA_DIR` 的受控 Sandbox 环境可用。
Production 安装态会返回稳定错误：

```text
sandbox_batch_production_forbidden
```

因此，多选 UI 落地和 Production 批量写入开放必须是两个独立切片。

### 前端门禁

前端不能读取环境变量或根据路径猜测是否处于 Sandbox。后端必须通过窄 typed capability 投影告诉前端：

```ts
type BatchModLifecycleCapability = {
  previewAvailable: boolean;
  writeAvailable: boolean;
  unavailableReasonCode: string | null;
};
```

具体字段可以复用未来统一 app capability snapshot，不要求为本文单独创建 command，但必须满足：

- capability 由 backend/runtime 决定。
- Production 未开放时，批量写按钮禁用并显示清晰原因。
- UI 不先调用写 command 再把 `sandbox_batch_production_forbidden` 当普通交互反馈。
- 即使 capability 显示可用，preview/apply 仍需后端逐次重验。

多选本身可以在 Production 使用，例如组织和查看选择；不可用的是批量写入动作，不是选择状态。

### Production 开放条件

Production 批量写入必须另行完成并 review，至少包括：

- Production runtime composition 接入既有 batch app service。
- command-level cross-process write admission。
- game/profile 写锁和锁内 facts 重验。
- 真实安装态的 Tauri DTO、stable error 和 task event contract。
- disposable Windows Sandbox 或等价隔离环境下的安装态人工验收。
- 单项、批量和 worker/CLI 竞争时的 fail-closed 行为。

本文不把 CLI-3A、Gate C 或现有 Sandbox UI 认证解释为 Production 已开放。

## 前端、Tauri 与后端职责

### 前端负责

- `single | batch` 模式和 `selectedIds` UI 状态。
- 卡片、快捷操作、计数、提示、focus、键盘和响应式行为。
- 将用户操作映射为明确 selection intent。
- 调用 feature-local typed API 并展示稳定 code 对应文案。
- 在 query context 变化时清空选择和作废请求。

前端不负责：

- 计算最终安装路径或 target claim。
- 判断 manifest、backup、rollback、recovery 的真实状态。
- 决定哪些失败可以 retry。
- 生成 plan token、batch id、attempt 或写入权限。
- 循环调用单项写 command。

### Tauri 负责

- 校验 DTO 形状和有界数量。
- 把 capability、preview、result 和 stable error 投影给前端。
- 调用应用层 batch service，不实现选择 UI 或文件写入规则。
- 保持 token、路径、manifest 正文和原始错误不进入日志或普通 DTO。

### 应用层与领域层负责

- 读取 exact revision、manifest、recovery、replacement target 和 prerequisite facts。
- 规范化 BatchPlan、执行上限、冲突、策略和 token。
- 串行化同 game/profile 写入并执行单项安全链。
- 保存 journal、结果和 retry identity。
- 返回可展示的聚合结果与稳定 code。

## 错误与通知

通知使用稳定 code 驱动，不解析自由文本。建议的前端本地选择 code：

```text
mod_selection_limit_reached
mod_selection_context_changed
mod_selection_cleared
mod_selection_locked_by_workflow
batch_mod_lifecycle_capability_unavailable
```

这些 code 只描述 UI 状态，不替代后端 batch error。通知原则：

- 上限和上下文清空使用低干扰 toast 或行内状态。
- global blocker、recovery required 和写入失败进入 preview/result 面板。
- 不为同一个搜索输入连续弹出重复 toast。
- 不展示路径、token、用户名、Steam ID、manifest 正文或原始错误。

## 并发、任务与安全约束

- 选择变化必须使未执行 preview 失效，不能复用旧 token。
- `starting/running` 期间 selection、profile、refresh 和 lifecycle controls 必须冻结。
- 同 game/profile 的真实写入继续由后端串行，UI 禁用不是并发正确性的来源。
- Batch runner 逐项复用单项安全事务，不声明整个批次全局原子。
- 已成功项不因后项失败伪造回滚；结果面板必须反映真实 partial result。
- game 正在运行、事实未知、recovery 未收敛或 admission 不可用时 fail closed。
- 取消只在后端安全点生效，不能从 UI 强行中断 commit、manifest 或 rollback 收尾。

## 响应式与动效

### 尺寸与布局

- 批量模式入口放在“快捷操作”标题栏，不挤压搜索框。
- 选择计数、选择命令和生命周期命令分组，窄屏可以换行但不出现横向滚动条。
- 卡片选择指示器使用固定尺寸和定位，hover、focus、选中和加载状态不能改变卡片几何尺寸。
- 最长文案在 `1280x800`、`1366x768` 和 `1440x900` 下不溢出。
- 触摸环境依赖显式批量入口，不依赖 Ctrl。

### 动效

- 进入批量模式时，选择指示器使用短 opacity/scale 过渡，不让卡片整体重新排版。
- toggle 只动画指示器、边框和背景，不移动标题或状态徽标。
- 达到上限时使用轻微状态反馈，不使用导致卡片位移的抖动。
- `prefers-reduced-motion: reduce` 下关闭非必要过渡。

## 键盘与焦点规则

| 按键 | 普通模式 | 批量模式 |
| --- | --- | --- |
| `Tab` | 在工具栏和卡片间移动焦点 | 同左 |
| `Enter` | 普通选择聚焦卡片 | toggle 聚焦卡片 |
| `Space` | 普通选择聚焦卡片 | toggle 聚焦卡片 |
| `Ctrl + Space` | 进入批量模式并 toggle | toggle |
| `Escape` | 关闭最上层菜单/对话框 | 先关闭最上层 UI；没有上层 UI 时退出批量模式 |

Windows 中文输入法可能占用 `Ctrl + Space`。该组合键是补充快捷方式，不是唯一入口；显式“批量选择”
按钮和批量模式内的普通 `Space`必须始终可用。

模式切换后焦点规则：

- 点击标题栏入口进入批量模式时，焦点保留在模式按钮。
- `Ctrl + 点击`进入时，焦点留在目标卡片。
- 清空选择不重置滚动位置。
- 因查询上下文变化清空时，焦点留在触发变化的搜索框、筛选按钮或刷新按钮。
- 成功结果关闭后，焦点回到“批量选择”入口。

## 测试矩阵

### 纯状态测试

- 普通 replace 只保留目标卡片。
- 普通点击唯一已选卡片会取消。
- 首次 Ctrl toggle 保留原单选并进入批量模式。
- Ctrl toggle 已选项会取消，模式不退出。
- 批量模式普通点击使用 toggle。
- 取消最后一项后仍保持批量模式。
- 退出批量模式清空选择和未执行 preview。
- 第 101 项被拒绝，取消已有项仍可执行。
- “选择本页”和“反选本页”超过上限时保持原集合。

### 页面状态测试

- 翻页和修改 page size 保留跨页选择。
- 搜索、筛选、配置档切换和刷新清空选择并只通知一次。
- 切换视图和分类标签显示不清空选择。
- 批量模式 1 项仍调用 batch workflow，普通模式 1 项调用单项 workflow。
- preview 关闭保留选择；全部成功关闭结果后清空并退出。
- partial result 保留 retry UI，retry item 由后端结果决定。
- workflow 运行中卡片、查询和配置档操作被冻结。

### 组件与可访问性测试

- 四种视图都有固定选择指示器。
- 普通模式使用 `aria-pressed`，批量模式使用 `aria-checked`。
- `Ctrl + 点击`和 `Ctrl + Space`产生明确 selection intent。
- 纯选择 helper 不依赖 React event。
- 计数和上限提示通过 live region 可读。
- 右键菜单在批量模式不改写选择集合。
- hover/focus 不隐藏卡片标题或状态文字。

### Contract 与安全测试

- 前后端都拒绝超过 100 项。
- Production capability 未开放时按钮禁用，直接 command 仍返回
  `sandbox_batch_production_forbidden`。
- stale preview、profile 变化和 selection 变化不能启动写入。
- progress/result 关联 task id、batch id 和 attempt number。
- 日志和 DTO 不含路径、token、manifest 正文或原始错误。

### 视觉验收

至少覆盖：

- `1440x900`、`1366x768`、`1280x800`。
- 浅色、深色和跟随系统。
- classic、grid、list、tech 四种视图。
- 普通 0/1 项、批量 0/1/多项、100 项上限。
- 工具栏换行、跨页计数、禁用原因、preview 和 partial result。
- 键盘 focus ring 与 reduced motion。

## 实施切片

### Slice A：选择状态与卡片意图

- 增加显式 `single | batch` 状态。
- 把卡片回调改为明确 selection intent。
- 接入 `Ctrl + 点击`、`Ctrl + Space`和批量模式 Space。
- 补纯状态和卡片交互测试。

### Slice B：快捷操作与跨页规则

- 增加 `ListChecks + 批量选择`入口。
- 只在批量模式展示选择本页、反选本页、清空和批量操作。
- 增加全局/本页计数和 100 项上限。
- 落实翻页保留、查询上下文变化清空和通知。

### Slice C：Workflow 分派与 Capability

- 让模式而不是数量决定 single/batch workflow。
- 保留现有 preview、confirm、result 和 retry。
- 增加 backend-owned capability 投影或复用统一 capability snapshot。
- Production 未开放时禁用批量写按钮并展示稳定原因。

### Slice D：可访问性与视觉验收

- 统一四种视图的选择指示器和 ARIA 语义。
- 覆盖键盘、右键菜单、focus、reduced motion 和响应式。
- 完成前端聚焦测试、build 和多 viewport smoke。

### Slice E：Production 批量写入开放

这是独立高风险任务，不属于前四个 UI 切片。需要单独设计、实现、完整验证和 disposable Windows
安装态验收，不能因多选 UI 完成而自动开放。

## 完成定义

设计对应实现只有同时满足以下条件才算完成：

1. 用户可以通过显式入口或 `Ctrl + 点击`进入批量模式。
2. 松开 Ctrl、取消到 0/1 项或翻页都不会意外退出批量模式。
3. 搜索、筛选、配置档切换和刷新会清空选择并给出一次明确反馈。
4. 四种视图的选择指示、键盘和屏幕阅读器语义一致。
5. 第 101 项无法进入选择集合，后端仍保留独立上限校验。
6. 批量模式中的 1..100 项全部走现有 batch preview/result/retry 闭环。
7. 混合状态项不会被前端静默跳过，后端 preview 仍是权威判断。
8. Production 未开放时不会向用户展示可执行的批量写入口。
9. 没有前端文件系统规则、单项 command 循环或敏感事实泄漏。
10. 聚焦测试、文档检查和指定 viewport 人工验收通过。

## 已冻结的交互决定

- 显式“批量选择”与 `Ctrl + 点击`共用同一状态机。
- 普通点击保持单选，批量模式中的普通点击执行 toggle。
- Ctrl 只负责快捷进入，不是必须持续按住的临时模式。
- 取消最后一项后仍保持批量模式。
- 退出批量模式时清空选择。
- 翻页和 page size 变化保留选择；搜索、筛选、配置档切换和刷新清空选择。
- 选择上限为 100，超过上限的集合操作整体拒绝，不静默截断。
- “选择本页”不等于“选择全部匹配结果”。
- 批量模式即使只有 1 项也走 batch workflow。
- Production capability 开放与前端多选实现分开交付。
