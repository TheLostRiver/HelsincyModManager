# 宽屏响应式布局设计

## 背景

当前 `AppFrame` 使用固定 `max-width: 1920px` 并居中显示。在 3840x2160、3440x1440、3840x1600 等宽屏或带鱼屏环境中，主工作区会被限制在中间，左右出现大块空白。这个问题不是单个页面的视觉瑕疵，而是应用外壳缺少大屏响应式策略。

本设计选择方案 B：分级宽屏密度系统。目标是在不重做页面结构的前提下，让工作台在常见桌面、2K、4K 和超宽屏上逐级提升空间利用率，同时保持工具型应用的可扫描性。

## 目标

- 4K 屏幕下减少左右空白，避免工作区被硬限制在 1920px。
- 2K、4K、21:9、32:9 环境下提升 Mod 管理页的信息密度。
- 保持 1366x768、1440x900、1920x1080 的现有视觉基线，不引入明显回退。
- 让宽屏策略通过布局 token 和少量断点表达，避免在页面里散落魔法数字。
- 保持当前前端边界：只调整 UI 布局和 CSS，不触碰安装、回滚、文件系统、游戏适配器或玩家数据逻辑。

## 非目标

- 不做完整工作台三栏重设计。
- 不新增 Mod 详情面板、预览面板或筛选侧栏。
- 不改变导航结构、路由状态机或侧边栏模式语义。
- 不引入用户可配置的布局密度开关。
- 不为填满超宽屏而无限放大字体、按钮或卡片。

## 现状约束

关键约束点：

- `src/app/frame/AppFrame.css`
  - `.app-shell` 当前固定 `max-width: 1920px`。
  - `.app-surface` 使用固定 `padding: var(--space-page)` 和 `gap: var(--space-content-gap)`。
- `src/app/routing/RouterOutlet.css`
  - `.route-transition__layer` 使用 `grid-template-columns: minmax(0, 1fr) 360px`。
  - Mod 管理页通过 `.route-transition__layer > .mod-library { grid-column: 1 / -1; }` 独占整行。
- `src/features/mods/ModLibraryPage.css`
  - `.mod-library__body` 使用 `minmax(0, 1fr) 168px`。
  - `.mod-grid` 使用 `repeat(auto-fill, minmax(200px, 1fr))`。
  - 小屏下已有 `1280px`、`960px`、`640px` 断点。

这些约束说明问题需要从全局外壳和 Mod 页面密度两个层级一起处理；只调整 Mod 网格无法解决 4K 左右空白，只删除 `max-width` 又会让内容横向失控。

## 设计原则

1. 外壳先扩展，内容再分级增密。
   宽屏首先扩大应用可用画布，然后由页面决定如何增加列数、面板宽度和间距。

2. 最大宽度仍然存在。
   工具型应用需要扫描效率。即使在 32:9 上，也不应让顶部状态栏、搜索栏或单行内容无限拉长。

3. 宽屏断点向上增强，小屏规则不回退。
   当前移动和中等桌面规则继续以 `max-width` 断点工作；新增宽屏规则使用 `min-width` 断点。

4. 通过 token 表达布局意图。
   页面可以消费 token，但不应各自定义互相冲突的超宽屏尺寸。

## 布局 Token

在全局样式 token 层新增或补充以下变量：

```css
:root {
  --layout-shell-max-width: 1920px;
  --layout-page-padding: var(--space-page);
  --layout-content-gap: var(--space-content-gap);
  --layout-route-aside-width: 360px;
  --layout-mod-action-panel-width: 168px;
  --layout-mod-card-min-width: 200px;
  --layout-mod-card-poster-height: 268px;
}
```

宽屏断点逐级覆盖这些变量：

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

具体数值可以在实现时通过视觉验证微调，但断点语义应保持稳定：

- `<= 1920px`：现有桌面基线。
- `1921px - 2560px`：2K 或 4K 高缩放视口。
- `2561px - 3200px`：4K 常见浏览器视口。
- `> 3200px`：超宽屏或 4K 低缩放视口。

## App Shell 设计

`AppFrame` 使用 token 控制最大宽度和内边距：

```css
.app-shell {
  max-width: var(--layout-shell-max-width);
}

.app-surface {
  gap: var(--layout-content-gap);
  padding: var(--layout-page-padding);
}
```

保留 `margin: 0 auto`。这样在 3840px 宽度下不再被锁死在 1920px，同时仍然允许极宽屏保留一定边界。

侧边栏宽度暂不变化。经典侧边栏 240px 在宽屏下仍然可用，浮动侧边栏也不需要为宽屏单独放大。

## Route Layer 设计

`RouterOutlet` 的通用 route layer 应消费 route aside token：

```css
.route-transition__layer {
  grid-template-columns: minmax(0, 1fr) var(--layout-route-aside-width);
}
```

本次不改变 route transition 状态机。只确保 route layer 的宽屏扩展不破坏页面布局。

需要保留以下规则：

```css
.route-transition__layer > .mod-library {
  grid-column: 1 / -1;
}
```

原因是 Mod 管理页自带右侧快捷操作面板，不应再被 route layer 的通用双列布局拆分。

## Mod 管理页设计

Mod 管理页使用全局 token 控制右侧操作面板和卡片网格：

```css
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

宽屏下的预期效果：

- 2K 视口：主内容略宽，卡片列数自然增加。
- 4K 视口：应用画布扩展到约 2880px，Mod 卡片保持合理尺寸，一屏展示更多卡片。
- 21:9 视口：减少左右空白，但保留操作面板和卡片尺寸的可读边界。
- 32:9 视口：最多进入 3200px 上限，不强行铺满整个横向空间。

小屏下现有规则继续覆盖：

- `max-width: 1280px` 时右侧快捷操作面板折叠为横向操作区。
- `max-width: 960px` 和 `max-width: 640px` 继续降低卡片最小宽度和海报高度。

## Dashboard 与其他页面影响边界

Dashboard 当前依赖 route layer 的通用双列布局。实现时需要确认：

- Dashboard 顶部状态栏不会在宽屏下产生不可读的超长单行。
- Dashboard 主内容和右侧状态面板仍然按现有语义排列。
- 其他占位页面不会因为 shell 放宽而出现横向滚动。

如果某些页面在宽屏下显得空，应优先接受“暂时空”，不为未完成页面添加装饰性填充。

## 断点验收矩阵

实现后需要用浏览器至少验证以下视口：

| 视口 | 用途 | 预期 |
| --- | --- | --- |
| 1366x768 | 常见小笔记本 | 无横向滚动，侧边栏和状态栏可用 |
| 1440x900 | 常见桌面 | 基本保持当前视觉基线 |
| 1920x1080 | Full HD | 不比当前更松散 |
| 2560x1440 | 2K | 工作区放宽，Mod 卡片增加列数 |
| 3440x1440 | 21:9 | 左右空白明显减少，内容不被无限拉长 |
| 3840x1600 | 24:10 / 超宽 | 工作区进入宽屏上限，操作面板仍可扫描 |
| 3840x2160 | 4K | 不再出现 1920px 硬限制造成的大块空白 |
| 3840x2160 @ 50% 浏览器缩放 | 4K 低缩放工作流 | 外壳仍居中且受上限约束，卡片、骨架屏、操作区保持同一栅格节奏 |

## 缩放韧性

4K 屏幕配合 50% 浏览器缩放时，浏览器会暴露接近超宽桌面的 CSS 视口。这个场景不应被当作特殊皮肤处理，而应由同一套外壳上限、布局 token 和网格规则自然覆盖。

验收重点不是完全填满整块屏幕，而是保持结构稳定：

- `.app-shell` 可以进入宽屏上限，但不能无限扩张到整块 4K 画布。
- 顶部状态、侧边栏、搜索和页面主体仍按原有层级排列，不出现模块漂移或横向滚动。
- Mod 卡片和加载骨架屏使用同一套网格宽度，不出现卡片一套列宽、骨架屏另一套列宽的割裂感。
- 右侧快捷操作区保持固定语义宽度，只随 token 小幅增宽，不因为缩放而变成大面积空白面板。
- 文本、状态 pill、按钮和卡片标题不重叠，不依赖负间距或视口字体缩放来“挤进去”。

## 自动化验证

前端实现完成后执行：

```powershell
cmd /c corepack pnpm run test
cmd /c corepack pnpm run typecheck
cmd /c corepack pnpm run lint
cmd /c corepack pnpm run build
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1
```

视觉验证使用浏览器截图或 DOM 尺寸检查记录关键结果。至少需要确认：

- `.app-shell` 在宽屏视口下实际宽度大于 1920px。
- `.app-shell` 在超宽视口下不超过设计上限。
- 在等效 4K 低缩放视口下，`.app-shell` 仍居中且宽度不超过设计上限。
- Mod 管理页 `.mod-grid` 列数随视口增加。
- Mod 管理页加载骨架屏与真实卡片使用一致的网格节奏。
- Mod 卡片标题、状态 pill、右侧快捷操作按钮没有文本溢出或重叠。

## 风险与取舍

- 放宽外壳会影响所有页面。通过 token 和视觉矩阵控制范围，避免页面各自散写宽屏规则。
- 卡片列数增加可能让样例数据显得不够多。当前阶段接受底部留白，不用装饰性内容填充。
- 32:9 屏幕无法完全消灭所有空白。保留上限是为了避免搜索栏和状态条变成不可读的超长横幅。
- 文档与实现需要保持一致。后续如果引入详情面板或三栏工作台，应另开设计，而不是在本次方案里隐式扩张。

## 实施边界

建议实现改动集中在：

- `src/shared/styles/tokens.css`
- `src/app/frame/AppFrame.css`
- `src/app/routing/RouterOutlet.css`
- `src/features/mods/ModLibraryPage.css`

不建议修改：

- Tauri command
- Rust crates
- 安装计划、manifest、backup、rollback 逻辑
- 游戏适配器
- 路由状态机
- Mod 数据模型
