# 全视口响应式布局设计

## 背景

当前 `AppFrame` 使用固定 `max-width: 1920px` 并居中显示。在 3840x2160、3440x1440、3840x1600 等宽屏或带鱼屏环境中，主工作区会被限制在中间，左右出现大块空白；而在 4K 低缩放（50%/33%/25%）下，浏览器暴露的 CSS 视口会进一步放大到 7680px ~ 15360px，问题更严重。

原第一版设计只针对"宽屏放大"做了增强，但在评审中暴露了三类问题：

1. **缩小方向覆盖不足**：验收下限只到 `1366x768`，没有覆盖笔记本窄屏、分屏、手机宽度。
2. **隐藏的硬编码遗漏**：`Dashboard.css` 的 `.workbench-body`、`.setup-rail` 仍有 `360px` 硬编码，与方案想消除的"魔法数字"自相矛盾；原方案还漏看了 Dashboard 同时受 `.route-transition__layer` 与 `.workbench-body` 两层双列约束。
3. **横向溢出无兜底**：`.app-shell { overflow: hidden }` 会裁掉溢出内容而非让它安全换行/滚动；长文本（超长游戏名、状态文案）在窄屏下没有截断策略。

本设计选择**方案 B 升级版：分级密度 token + 全视口契约**。目标是在"放大不松散、缩小不破裂"两个方向都达到结构稳定，而非只解决宽屏一侧。

## 目标

- **放大方向**：4K / 带鱼屏 / 4K 低缩放下减少左右空白，外壳进入宽屏密度但不无限扩张。
- **缩小方向**：从 `1366px` 一路缩到 `375px`（手机宽度）乃至更窄的拖拽窗口，布局逐级降级、**不出现横向滚动、不裁切可交互内容**。
- **密度可调**：2K、4K、21:9、32:9 环境下逐级提升 Mod 管理页信息密度，同时保持工具型应用的可扫描性。
- **基线不回退**：`1366x768`、`1440x900`、`1920x1080` 的现有视觉与所有小屏断点契约保持不变。
- **布局意图集中**：所有宽屏尺寸通过 layout token 和断点表达，页面不散落魔法数字。
- **边界严守**：只调整 UI 布局和 CSS，不触碰安装、回滚、文件系统、游戏适配器或玩家数据逻辑。

## 非目标

- 不做完整工作台三栏重设计。
- 不新增 Mod 详情面板、预览面板或筛选侧栏。
- 不改变导航结构、路由状态机或侧边栏模式语义。
- 不引入用户可配置的布局密度开关。
- 不为填满超宽屏而无限放大字体、按钮或卡片。
- 不为 < 375px 的极端视口做像素级优化（只保证不破裂）。

## 能力边界

- 本方案承诺的是：在本文明确列出的页面、侧边栏模式、断点矩阵与验证方法内，达到结构稳定、可回归、可追踪。
- 本方案**不承诺**：仅凭若干抽样视口，就能证明"任意宽高、任意浏览器、任意未来页面"都像素级稳定。
- 对 `< 375px`、极端低高度窗口、浏览器最小字体/缩放怪癖等场景，本方案只要求 best-effort 不灾难性破裂；如果需要更强保证，必须单独扩充设计和验收矩阵。

## 现状约束（已核对源码）

真实 DOM 层级（来自 `AppFrame.tsx` + `RouterOutlet.tsx`）：

```
.app-shell
├── Sidebar（classic 240px 固定列 / floating 88px 浮动）
└── .app-surface
    ├── AppHeader（.top-status-bar）
    └── main.workbench-body
        └── .route-transition
            └── .route-transition__layer（每路由一个）
                └── 页面内容
```

关键约束点：

- `src/app/frame/AppFrame.css`
  - `.app-shell`：`max-width: 1920px`（硬编码），`overflow: hidden`（横向溢出会被**裁切**，是缩小方向最大风险源）。
  - `.app-surface`：`gap: var(--space-content-gap)`、`padding: var(--space-page)`、`overflow: auto`。
  - `.top-status-bar`：`grid-template-columns: minmax(220px, 1fr) auto auto`，固定 `height: 64px`。
  - `@media (max-width: 1360px)`：状态栏降级，隐藏 `.window-tools` 和非 compact `.status-pill`。
  - `@media (max-width: 860px)`：shell 单列化，surface padding 降到 16px。
- `src/app/routing/RouterOutlet.css`
  - `.route-transition__layer`：`grid-template-columns: minmax(0, 1fr) 360px`（硬编码）。
  - `@media (max-width: 1360px)`：layer 单列化。
- `src/features/dashboard/Dashboard.css`
  - `.workbench-body`：`grid-template-columns: minmax(0, 1fr) 360px`（硬编码，**Dashboard 真正消费的双列容器**）。
  - `.setup-rail`：`width: 360px`（硬编码）。
  - `@media (max-width: 1360px)`：workbench 单列化，rail 宽度放开。
  - `@media (max-width: 860px)`：support/preview/summary 网格降为单列。
- `src/features/mods/ModLibraryPage.css`
  - `.mod-library__body`：`minmax(0, 1fr) 168px`（硬编码）。
  - `.mod-grid`：`repeat(auto-fill, minmax(200px, 1fr))`（硬编码）。
  - `.mod-card__poster`：`height: 268px`（硬编码）。
  - 小屏断点：`1280px` / `960px` / `640px`。
- `src/app/shell/layouts/floating-sidebar/FloatingSidebar.css`
  - 浮动侧边栏 `88px`，`@media (max-width: 860px)` 缩到 `52px`，`@media (max-height: 820px)` 还有纵向断点。

**重要结论**：`.route-transition__layer` 和 `.workbench-body` 是**两个并行的双列容器**。`DashboardPage.tsx` 返回 fragment，其 `.main-workspace` + `SetupStatusPanel` 两个节点会落进同一个 `.route-transition__layer`，因此 layer 的双列规则对 Dashboard **确实生效**；但 `.workbench-body` 的 `360px` 是更外层的、对所有页面生效的第二个 `360px`。两处必须**同时 token 化**，否则 Dashboard 宽屏下右侧 rail 仍是固定 360px、且与 layer 规则可能叠加冲突。

## 设计原则

1. **外壳先扩展，内容再分级增密。**
   宽屏首先扩大应用可用画布，然后由页面决定如何增加列数、面板宽度和间距。

2. **最大宽度仍然存在。**
   工具型应用需要扫描效率。即使在 32:9 上，也不应让顶部状态栏、搜索栏或单行内容无限拉长。

3. **双向断点，向上增强不向下回退。**
   现有 `max-width` 断点全部保留并锁为契约；新增宽屏用 `min-width`。`1361px–1920px` 段由 `:root` 基线兜底（即 1920px 封顶 + token 默认值）。

4. **缩小方向优先保证"不破裂"。**
   窄视口下允许密度降低、空白增加，但不允许横向滚动、内容裁切、可交互元素消失或重叠。

5. **通过 token 表达布局意图。**
   页面可以消费 token，但不应各自定义互相冲突的尺寸。

6. **长文本永远有兜底。**
   任何可能承载用户输入文本（游戏名、状态文案、文件路径、Mod 标题）的元素必须配置截断或换行策略，不依赖视口宽度"恰好够用"。

## 布局 Token

在全局样式 token 层新增以下变量（基础值与现状对齐，保证基线零变化）：

```css
:root,
:root[data-color-scheme="light"] {
  /* 布局 token：宽屏密度 */
  --layout-shell-max-width: 1920px;
  --layout-page-padding: var(--space-page);
  --layout-content-gap: var(--space-content-gap);
  --layout-route-aside-width: 360px;        /* route layer + workbench 共用 */
  --layout-mod-action-panel-width: 168px;
  --layout-mod-card-min-width: 200px;
  --layout-mod-card-poster-height: 268px;
  /* 文本兜底 token */
  --layout-text-overflow: ellipsis;          /* 默认单行截断语义 */
}
```

宽屏断点逐级覆盖密度 token（只改密度，不改文本兜底）：

```css
@media (min-width: 1921px) {
  :root {
    --layout-shell-max-width: 2400px;
    --layout-mod-action-panel-width: 192px;
    --layout-mod-card-min-width: 210px;
  }
}

@media (min-width: 2561px) {
  :root {
    --layout-shell-max-width: 2880px;
    --layout-page-padding: 36px;
    --layout-content-gap: 24px;
    --layout-mod-action-panel-width: 208px;
    --layout-mod-card-min-width: 220px;
  }
}

@media (min-width: 3201px) {
  :root {
    --layout-shell-max-width: min(100vw, 3200px);
    --layout-page-padding: 40px;
    --layout-content-gap: 28px;
    --layout-mod-action-panel-width: 220px;
    --layout-mod-card-min-width: 230px;
  }
}
```

断点语义（数值可在实现时通过视觉验证微调，但断点位置和方向不可变）：

| 断点 | 方向 | 适用区间 | 语义 |
| --- | --- | --- | --- |
| `max-width: 560px` | 向下 | `<= 560px` | 超窄：导航单列、setup panel padding 收紧 |
| `max-width: 640px` | 向下 | `<= 640px` | 手机宽：Mod 卡片最小宽度 150px、poster 220px |
| `max-width: 860px` | 向下 | `<= 860px` | shell 单列化、浮动侧栏缩到 52px |
| `max-width: 960px` | 向下 | `<= 960px` | Mod 卡片最小宽度 170px |
| `max-width: 1280px` | 向下 | `<= 1280px` | Mod 操作面板折叠为横向滚动区 |
| `max-width: 1360px` | 向下 | `<= 1360px` | 状态栏降级、route/workbench 单列化 |
| 基线 | — | `1361px – 1920px` | `:root` 默认值，shell 封顶 1920px |
| `min-width: 1921px` | 向上 | `1921px – 2560px` | shell 2400px，Mod 密度 +1 档 |
| `min-width: 2561px` | 向上 | `2561px – 3200px` | shell 2880px，间距/密度提升 |
| `min-width: 3201px` | 向上 | `> 3200px` | shell 封顶 `min(100vw, 3200px)` |

## App Shell 设计

`AppFrame` 消费 token 控制最大宽度和内边距：

```css
.app-shell {
  /* max-width 改为 token；其余声明保持不变 */
  max-width: var(--layout-shell-max-width);
}

.app-surface {
  gap: var(--layout-content-gap);
  padding: var(--layout-page-padding);
}
```

保留 `margin: 0 auto`。侧边栏宽度（classic 240px / floating 88px）暂不变化，宽屏下仍可用。

### 横向溢出策略（缩小方向关键）

现状 `.app-shell { overflow: hidden }` 会裁切溢出内容。本设计**不改变**这条规则（裁切比横向滚动条体验好），而是通过下面三道防线保证内容**不会真正溢出**：

1. **所有 flex/grid 子项强制 `min-width: 0`**：防止 flex/grid item 因内容最小宽度撑爆容器。
2. **长文本元素统一截断**：见下节"长文本兜底"。
3. **窄屏断点逐级降级**：到 `860px` shell 单列化后，剩余横向压力由各页面的小屏规则吸收。

如果实现中仍发现某元素溢出，**优先在责任元素上加 `min-width: 0` + 截断**，而非放宽 shell 的 `overflow`。

### `min-width: 0` 关键审计清单

以下容器是本方案定义的**首批必审计对象**。它们承接壳体、双列、卡片区或长文本压力；若缺少 `min-width: 0`，最容易在保留 `overflow: hidden` 的前提下被内容撑爆：

| 选择器 | 所在文件 | 责任 | 当前状态 |
| --- | --- | --- | --- |
| `.app-surface` | `AppFrame.css` | 壳体内部主内容容器 | 已有 |
| `.top-status-bar` | `AppFrame.css` | 承接长游戏名与状态胶囊 | 已有 |
| `.current-game` | `AppFrame.css` | 长游戏名截断链路 | 已有 |
| `.route-transition` | `RouterOutlet.css` | 路由层外壳 | 已有 |
| `.route-transition__layer` | `RouterOutlet.css` | 通用双列容器 | 已有 |
| `.workbench-body` | `Dashboard.css` | Dashboard / 全局工作区双列容器 | **需补查** |
| `.main-workspace` | `Dashboard.css` | Dashboard 主内容区 | 已有 |
| `.setup-rail` | `Dashboard.css` | Dashboard 右侧 rail | 已有 |
| `.mod-library` | `ModLibraryPage.css` | Mod 页面根容器 | 已有 |
| `.mod-library__body` | `ModLibraryPage.css` | Mod 主内容 + 操作面板双列 | 已有 |
| `.mod-library__main` | `ModLibraryPage.css` | Mod 主卡片区 | 已有 |
| `.compact-panel` | `ModLibraryPage.css` | 小屏折叠后操作区 | **需补查** |
| `.compact-panel__stack` | `ModLibraryPage.css` | 横向滚动操作栈 | 已有 |
| `.compact-action__left` | `ModLibraryPage.css` | 按钮文本截断链路 | 已有 |

说明：

- `已/需补查` 仅表示文档设计时的首轮源码核对结论；实现前应再核对一次最新源码。
- 这张表不是全仓最终穷举，而是本次响应式改动范围内的**强制检查集**。若实现中发现新的承压容器，应追加到该表，而不是只在代码里默默修。
- 实现完成后，验收记录应明确写出：哪些容器已核对、哪些容器补了 `min-width: 0`、哪些容器确认不需要。
- **语义区分：不要把 `min-width: 0` 理解成“越多越好”或“滚动容器一律不能写”。** 它的职责是让 flex/grid item 在承压时允许收缩，避免被内容最小宽度撑爆；是否需要保留 `auto`，取决于该容器在父级布局里承担的是“收缩责任”还是“内容内在宽度责任”。对像 `.compact-panel__stack` 这类既是横向滚动区、又处在 flex/grid 收缩链路里的容器，不能凭语义猜测直接否定 `min-width: 0`，而应以真实渲染验证为准。本方案当前口径是：`.compact-panel__stack` 继续纳入审计清单并保留现有 `min-width: 0`，再由小屏 L3 验收确认横向滚动区未塌缩、滚动内容完整可见。

## 长文本兜底

定义两种语义，按内容性质选择：

- **单行截断**（游戏名、状态文案、文件路径、按钮标签等"标签型"内容）：

  ```css
  .truncate-text {
    min-width: 0;
    overflow: hidden;
    text-overflow: var(--layout-text-overflow);
    white-space: nowrap;
  }
  ```

  现状已具备的元素（`AppFrame.css` 的 `.current-game strong`、`.status-pill strong`、`ModLibraryPage.css` 的 `.compact-action__label`）保持，并补查 `.current-game strong` 在超长游戏名下是否真的能触发截断（需父链路都有 `min-width: 0`）。

- **多行换行**（描述性段落、错误信息等"正文型"内容）：

  ```css
  .wrap-text {
    min-width: 0;
    overflow-wrap: anywhere;
  }
  ```

  现状 `.mod-card__title`、`.setup-message h3/p`、Dashboard 各 `overflow-wrap: anywhere` 元素已符合。

本设计不新增装饰性 class，只确保所有承载用户文本的容器命中上述两种语义之一。

## Route Layer 与 Workbench 设计

两个并行双列容器**都消费同一个 token**，避免规则分裂：

```css
/* RouterOutlet.css */
.route-transition__layer {
  grid-template-columns: minmax(0, 1fr) var(--layout-route-aside-width);
}

/* Dashboard.css */
.workbench-body {
  grid-template-columns: minmax(0, 1fr) var(--layout-route-aside-width);
}

.setup-rail {
  width: var(--layout-route-aside-width);
}
```

**澄清 `.workbench-body` 与 `.route-transition__layer` 的关系**：Dashboard 页面的两个子节点会落进 layer 的双列，而 workbench 是 layer 外层、对所有页面生效的第二个双列容器。对 Dashboard 而言，真正决定右侧栏宽度的是**内层 layer**（因为它先匹配两个直接子节点）；workbench 的双列只对"路由层整体作为单一子节点"时生效。为避免歧义，两处都用同一 token，保证无论谁生效都是 `360px` → 宽屏同步。

保留：

```css
.route-transition__layer > .mod-library {
  grid-column: 1 / -1;
}
.workbench-body > .mod-library {
  grid-column: 1 / -1;
}
```

Mod 管理页自带右侧操作面板，不应被任何双列布局拆分。

## Dashboard 宽屏影响边界

Dashboard 是本设计**新增**的收口对象。实现时确认：

- `.workbench-body` 与 `.setup-rail` 的 `360px` 已 token 化（统一为 `--layout-route-aside-width`），消除硬编码。但宽屏断点**刻意不覆盖** `--layout-route-aside-width`，所以 Dashboard 右侧状态栏在所有视口下保持 `360px`，不随宽屏放大。
  - 这是刻意取舍：Dashboard 是低频状态信息，不需要像 Mod 卡片那样增密；保持 360px 可避免状态栏在超宽屏被拉成空面板。token 化的目的只是"消除散落的魔法数字 + 让 route layer 与 workbench 用同一把尺子"，不是"让 Dashboard 增密"。
- `.main-header h2`（26px）、`.setup-message h3`（28px）在宽屏下**不放大**，避免标题条变超长横幅。
- Dashboard 各 `auto-fit` 网格（`.support-grid`、`.preview-grid`、`.summary-grid`）在宽屏下自然增加列数，无需额外规则。
- `.setup-panel { min-height: 360px }` 与布局无关（是内容最小高度），保持。

## Mod 管理页设计

Mod 管理页消费全局 token 控制右侧操作面板和卡片网格：

```css
.mod-library {
  gap: var(--layout-content-gap);
}

.mod-library__body {
  grid-template-columns: minmax(0, 1fr) var(--layout-mod-action-panel-width);
  gap: var(--layout-content-gap);
}

.mod-grid {
  grid-template-columns: repeat(auto-fill, minmax(var(--layout-mod-card-min-width), 1fr));
}

.mod-card__poster {
  height: var(--layout-mod-card-poster-height);
}
```

宽屏预期效果：

- 2K：主内容略宽，卡片列数自然增加。
- 4K：画布扩展到约 2880px，一屏展示更多卡片，卡片尺寸保持合理。
- 21:9 / 32:9：进入 3200px 上限，不强行铺满。

小屏下现有规则保留并以局部 token 覆盖表达（语义不变）：

- `max-width: 1280px`：右侧操作面板折叠为横向操作区。
- `max-width: 960px`：`.mod-library { --layout-mod-card-min-width: 170px }`。
- `max-width: 640px`：`.mod-library { --layout-mod-card-min-width: 150px; --layout-mod-card-poster-height: 220px }`。

## 缩放韧性

### 放大方向（4K 低缩放）

4K 屏幕配合 50%、33%、25% 浏览器缩放时，CSS 视口会放大到 7680 / 11636 / 15360px。由 `min-width: 3201px` 断点 + `min(100vw, 3200px)` 上限自然覆盖。**50%、33%、25% 都是必测档，不允许只验证其中一个推断其他**。

验收重点不是填满屏幕，而是结构稳定：

- `.app-shell` 可进入宽屏上限，但不无限扩张到整块 4K 画布。
- 顶部状态、侧边栏、搜索、页面主体仍按原层级排列，不漂移、不横向滚动。
- Mod 卡片与骨架屏（若存在）使用同一套网格节奏。
- 右侧操作区保持固定语义宽度，只随 token 小幅增宽。
- 文本、状态 pill、按钮、卡片标题不重叠、不依赖负间距。
- 极端低缩放下允许外壳外侧保留空白。

### 缩小方向（窗口拖拽 / 窄屏）

从 `1366px` 缩到 `375px` 乃至更窄，要求：

- **无横向滚动**：`document.documentElement.scrollWidth <= clientWidth` 在所有目标视口成立。
- **无内容裁切**：可交互元素（按钮、链接、输入框）不被 `overflow: hidden` 切掉可见区域。
- **逐级降级**：每个 `max-width` 断点按表触发，不跳档、不叠加。
- **小屏契约不破坏**：见下节。

工程目标是连续拖拽时平滑降级；但本文只对关键宽度给出强验收点。**离散抽样点不能等同于对任意宽度的严格证明**，因此实现验收还需要补充连续拖拽观察和低高度窗口检查。

## 小屏契约（负向保护）

以下现有行为是**不可破坏的契约**，宽屏改动不得覆盖：

| 视口 | 契约行为 | 出处 |
| --- | --- | --- |
| `<= 1360px` | `.window-tools` 隐藏；非 compact `.status-pill` 隐藏 | `AppFrame.css:142-159` |
| `<= 1360px` | `.route-transition__layer` 单列化 | `RouterOutlet.css:60-65` |
| `<= 1360px` | `.workbench-body` 单列化；`.setup-rail` 宽度放开 | `Dashboard.css:619-628` |
| `<= 1280px` | `.mod-library__body` 单列化；`.compact-panel` 横向滚动 | `ModLibraryPage.css:541-566` |
| `<= 960px` | Mod 卡片最小宽度 170px | `ModLibraryPage.css:568-572` |
| `<= 860px` | `.app-shell` 单列化（非浮动模式）；surface padding 16px | `AppFrame.css:161-183` |
| `<= 860px` | `.sidebar` 变底部栏；`.nav-list` 两列 | `ClassicSidebar.css:147-161` |
| `<= 860px` | `.floating-sidebar` 缩到 52px | `FloatingSidebar.css:95-116` |
| `<= 640px` | Mod 卡片最小宽度 150px、poster 220px；compact-panel 单列 | `ModLibraryPage.css:574-594` |
| `<= 560px` | `.nav-list` 单列；`.setup-panel` padding 20px | `ClassicSidebar.css:163`、`Dashboard.css:638` |

## 断点验收矩阵

实现后必须用浏览器/DevTools 设备模拟验证以下全部视口：

| 视口 | 用途 | 预期 |
| --- | --- | --- |
| `375x812` | 手机宽度 | 无横向滚动，shell 单列，导航可用，setup panel 不溢出 |
| `800x600` | 超窄窗口/旧屏 | 无横向滚动，状态栏降级正常 |
| `1024x768` | 小笔记本/分屏 | 无横向滚动，侧边栏和状态栏可用 |
| `1366x768` | 常见小笔记本 | 无横向滚动，基线视觉保持 |
| `1440x900` | 常见桌面 | 基本保持当前视觉基线 |
| `1920x1080` | Full HD | 不比当前更松散 |
| `2560x1440` | 2K | 工作区放宽（<=2400），Mod 卡片增加列数 |
| `3440x1440` | 21:9 | 左右空白明显减少，内容不横向失控（<=2880） |
| `3840x1600` | 超宽 | 工作区进入宽屏上限（<=3200），操作面板仍可扫描 |
| `3840x2160` | 4K | 不再出现 1920px 硬限制的大块空白（<=3200） |
| `7680x4320` | 等效 4K @ 50% | shell 居中且 <=3200 |
| `11636x6545` | 等效 4K @ 33% | shell 居中且 <=3200 |
| `15360x8640` | 等效 4K @ 25% | shell 居中且 <=3200 |

> 注：4K 低缩放的 CSS 视口实际值取决于浏览器实现（部分浏览器对极端缩放有上限或会触发降级）。**推荐用 DevTools 的 "Responsive" 设备模拟直接设置上述像素值**，而非依赖浏览器缩放快捷键——前者确定性更高。若用真实浏览器缩放，须记录实际 `window.innerWidth`。

每个视口需满足三条硬约束：

1. `hasHorizontalOverflow === false`
2. 无可交互元素被裁切（目视 + 可选 DOM 检查）
3. shell 宽度符合上表预期

### 非宽度维度补充验收

除上表外，还必须补充以下维度，否则不能声称"全视口"已经验收完成：

- **真实页面**：至少对 `Dashboard` 与 `/mods` 两条真实路由各跑一轮；`layout.fixture.html` 只能做辅助测量，不能替代真实页面验收。
- **侧边栏模式**：至少抽样覆盖 classic 与 floating 两种模式；因为两者的列结构、遮挡风险和小屏降级路径不同。
- **低高度窗口**：至少补测 `1280x720` 与 `1280x640`，确认 `100vh`、sticky 面板、浮动侧边栏高度断点不会让主要操作不可达。
- **连续拖拽**：至少执行一轮 `1366px -> 375px` 连续缩窗观察，确认没有在非断点位置突然出现裁切、重叠或交互控件消失。
- **焦点可达性**：至少抽样验证主要按钮、链接、输入框的键盘聚焦状态在 shell 可见区域内，不被裁切、不落入不可见容器。

## 风险与取舍

- **放宽外壳影响所有页面**：通过 token 和验收矩阵控制范围，页面不散写宽屏规则。
- **两个双列容器**：`.workbench-body` 与 `.route-transition__layer` 必须同步 token 化，否则 Dashboard 右侧栏宽屏下不随 token 变化（这是刻意取舍，不视为缺陷）。
- **卡片列数增加可能让样例数据显得不够多**：接受底部留白，不用装饰性内容填充。
- **32:9 无法完全消灭空白**：保留上限是为了避免搜索栏和状态条变超长横幅。
- **Dashboard 不在宽屏断点增密**：低频状态信息保持 360px，避免空面板。
- **< 375px 不做像素级优化**：只保证不破裂（无横向滚动、无裁切），不追求美观。
- **浏览器极端缩放不可控**：25% 缩放下浏览器自身行为（字体最小尺寸、滚动条）超出 CSS 能力范围，接受其降级，但 shell 上限必须成立。
- **保留 `.app-shell { overflow: hidden }`**：意味着稳定性依赖于责任元素上的 `min-width: 0`、截断与换行审计。矩阵通过能证明"当前验收范围内稳定"，不能自动推出"任意未来内容都安全"。
- **fixture 不是完整应用**：它适合测壳体宽度、列数与横向溢出，但不能完整覆盖真实路由、侧边栏模式切换、sticky 行为与低高度交互。
- **文档与实现一致**：后续若引入详情面板或三栏工作台，应另开设计，不在本方案隐式扩张。

## 实施边界

建议实现改动集中在：

- `src/shared/styles/tokens.css`（新增 layout token + 宽屏断点）
- `src/app/frame/AppFrame.css`（消费 shell/padding/gap token）
- `src/app/routing/RouterOutlet.css`（消费 route aside token）
- `src/features/dashboard/Dashboard.css`（消费 route aside token：workbench + setup-rail）
- `src/features/mods/ModLibraryPage.css`（消费 mod 密度 token + 小屏局部覆盖）

新增测试：

- `src/shared/styles/layoutTokens.test.mjs`（token 合约 + 小屏契约负向测试）
- `src/shared/styles/layout.fixture.html`（DOM 骨架，供可选的布局测量）

不建议修改：

- Tauri command、Rust crates
- 安装计划、manifest、backup、rollback 逻辑
- 游戏适配器
- 路由状态机
- Mod 数据模型
- 侧边栏组件结构与宽度（classic 240px / floating 88px）
