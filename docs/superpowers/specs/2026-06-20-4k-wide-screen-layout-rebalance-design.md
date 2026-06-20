# 4K 宽屏留白修复设计

## 背景

上一轮“全视口响应式布局”已经解决了两个关键问题：

- 小屏缩小时不再出现结构性横向炸裂。
- 宽屏不再被旧的 `1920px` 上限锁死。

但它仍然保留了一个明显的 4K 体验缺口：

- 在 `3840x2160 @ 100%` 这类正常 4K 桌面环境下，`App Shell` 最终仍封顶在 `3200px`。
- 这会在左右两侧留下约 `320px / 侧` 的可见空白。
- 对内容网站，这类留白可能是可接受的；但对 Helsincy Mod Manager 这种桌面工具型界面，它会让用户明显感到“屏幕没被吃起来”。

这不是代码失效，也不是响应式崩坏，而是当前宽屏策略仍偏保守。

用户通过临时可视化对比页确认偏好为：

- 不接受继续保留当前 4K 下的大留白。
- 也不希望采用近乎铺满的激进方案。
- 更偏好“明显更宽，但仍保留可感知安全边距”的受控扩展方向。

因此，本轮设计选择 `B2：受控扩展型 + mods 密度微增`。

## 目标

- 在 `3840x2160 @ 100%` 下明显减少左右留白，让界面更接近桌面工具应有的宽屏利用率。
- 保持一层可感知安全边距，避免顶部条、搜索区、卡片区被横向拉散。
- 让 `/mods` 页面在 4K 下不只是“更宽”，而是能多承载一部分有效信息密度。
- 保持 `dashboard` 的信息扫视稳定性，不把它错误改造成“超宽横幅页面”。
- 不破坏上一轮已经锁定的小屏契约、低高度契约和容器护栏。

## 非目标

- 不重做 App Shell 结构。
- 不引入三栏工作台、详情常驻栏或全局第二侧栏。
- 不修改 classic / floating sidebar 的语义和宽度。
- 不把宽屏策略做成用户可配置项。
- 不重做 `dashboard` 的 rail 架构或卡片编排。
- 不触碰 Tauri / Rust / 安装回滚 / 文件系统 / 适配器逻辑。

## 现状结论

当前宽屏实现的核心 token 在 `src/shared/styles/tokens.css`：

- `1921px+`：`--layout-shell-max-width: 2400px`
- `2561px+`：`--layout-shell-max-width: 2880px`
- `3201px+`：`--layout-shell-max-width: min(100vw, 3200px)`

当前问题不在小屏，而在超大屏：

- `3200px` 上限对 `3840px` 宽视口过紧。
- `mods` 页面虽然会随 shell 变宽多出一些列数，但当前超宽档位同时把 `gap`、`padding`、卡片最小宽度都继续拉大，导致“新增宽度”没有尽可能转化为有效密度。
- `dashboard` 的右侧 rail 不适合跟着大幅膨胀，否则会迅速变成视觉空面板。

## 设计原则

### 1. 先修“壳体利用率”，再修“页面密度”

本轮优先让最外层宽屏利用率达到更合理的工具型水位，然后只在 `mods` 这种高密度页面上做温和的二次增密。

### 2. 宽屏增强不等于无限铺满

用户已经否定了“近乎铺满型”。这意味着 4K 修复不是把 `App Shell` 推到接近 `3840px`，而是在明显减少留白的同时，继续保留一层视觉边界。

### 3. `/mods` 增密，`dashboard` 稳定

`/mods` 是最适合利用额外宽度的页面，因为它天然受益于更多卡片列和更好的列表扫描。

`dashboard` 不是。它应继续保持“工作台摘要页”的稳定感，而不是被拉成横向长条。

### 4. 不回退小屏契约

本轮只增强宽屏，不重写下降断点，不动已经通过验证的小屏防线：

- `1360px`
- `1280px`
- `960px`
- `860px`
- `640px`
- `560px`

## 选定方案：B2

### 方案定义

`B2` 由两部分组成：

1. `App Shell` 超宽上限继续放宽。
2. `/mods` 页面在超宽档位做轻微增密，而不是继续把 gap 和卡片尺寸同步放大。

### 目标观感

在 `3840x2160 @ 100%` 下：

- 当前方案：约 `320px / 侧` 留白。
- 本轮目标：收敛到约 `200px / 侧` 量级。

这意味着最终 `App Shell` 在 4K 下的目标上限应落在 `3440px` 左右，而不是继续停在 `3200px`，也不是激进走到 `3600px+`。

## 宽屏 token 设计

保留现有断点方向不变，只调整超宽档位数值。

### 全局 layout token

基础档与 `1921px+` 档位保持不变：

```css
:root,
:root[data-color-scheme="light"] {
  --layout-shell-max-width: 1920px;
  --layout-page-padding: var(--space-page);
  --layout-content-gap: var(--space-content-gap);
  --layout-route-aside-width: 360px;
  --layout-mod-action-panel-width: 168px;
  --layout-mod-card-min-width: 200px;
  --layout-mod-card-poster-height: 268px;
}

@media (min-width: 1921px) {
  :root {
    --layout-shell-max-width: 2400px;
    --layout-mod-action-panel-width: 192px;
    --layout-mod-card-min-width: 210px;
  }
}
```

调整 `2561px+` 与 `3201px+` 档位：

```css
@media (min-width: 2561px) {
  :root {
    --layout-shell-max-width: 3040px;
    --layout-page-padding: 32px;
    --layout-content-gap: 20px;
    --layout-mod-action-panel-width: 200px;
    --layout-mod-card-min-width: 208px;
  }
}

@media (min-width: 3201px) {
  :root {
    --layout-shell-max-width: min(100vw, 3440px);
    --layout-page-padding: 36px;
    --layout-content-gap: 22px;
    --layout-mod-action-panel-width: 212px;
    --layout-mod-card-min-width: 212px;
  }
}
```

## 为什么这样调

### `App Shell`

- `2400px` 档位保持不动，避免影响 `2560x1440` 一类 2K 基线过大。
- `2880px -> 3040px`：让 `2561px~3200px` 区间开始体现“第二轮宽屏优化”，但不至于突然跳太大。
- `3200px -> 3440px`：这是本轮最关键的 4K 修复，直接把 4K 左右空白压缩到更合理的水平。

### `mods` 密度

上一轮超宽档位的问题不是“没变宽”，而是“变宽的同时把卡片和间距也继续拉大”，导致部分新增宽度没有高效转化为内容密度。

本轮反向微调：

- `page padding` 不再继续拉到 `40px`，而收回到 `36px`
- `content gap` 不再继续拉到 `28px`，而收回到 `22px`
- `mod card min width` 不再继续升到 `230px`，而收敛在 `212px`

这样做的结果是：

- 页面看起来依然更宽。
- `/mods` 在 4K 下更容易多出 `1` 列有效卡片。
- 工具区不会因为卡片被过度放大而显得松散。

## 页面级行为

### App Shell

继续使用现有 token 消费结构：

- `AppFrame.css`
- `RouterOutlet.css`

本轮不改结构，只改 token 数值。

### `/mods`

`/mods` 是本轮唯一需要“随 4K 一起微增密”的页面。

具体表现：

- 右侧操作区宽度温和增加，但不抢占过多主内容空间。
- 卡片最小宽度不再继续放大。
- gap / padding 收紧后，更多横向空间会转化为实际列数。

### `dashboard`

`dashboard` 不做宽屏增密，只共享更宽的外层 shell。

保留原则：

- `--layout-route-aside-width` 继续固定为 `360px`
- `setup rail` 不跟着 4K 一路膨胀
- 不新增 dashboard 专用宽屏断点

这样可以避免：

- 右侧 rail 变成视觉空区
- dashboard 顶部和摘要区被横向拉散

## 验收标准

### 视觉目标

在 `3840x2160 @ 100%` 下：

- 左右空白不再维持当前“大块留白”观感
- 界面明显比当前版本更饱满
- 但仍能看出外层安全边距

### 结构目标

- 继续无横向滚动
- 继续无真实内容裁切
- 继续保留现有小屏下降契约
- `dashboard` 不出现“变成超宽空面板”的副作用

### `/mods` 页面目标

- 4K 下比当前版本多承载一部分有效卡片信息
- 不通过单纯放大卡片来消耗额外空间
- 维持可扫视性，不出现“一行太长、卡片太散”的观感

## 验证矩阵

本轮重点增加以下宽屏检查：

- `2560x1440`
- `3440x1440`
- `3840x1600`
- `3840x2160`

同时保留上一轮的小屏 / 低高度抽样：

- `1366x768`
- `1280x720`
- `1280x640`
- `800x600`
- `375x812`

真实页面至少覆盖：

- `Dashboard`
- `/mods`
- classic sidebar
- floating sidebar

## 实施边界

预计改动仍集中在以下文件：

- `src/shared/styles/tokens.css`
- `src/app/frame/AppFrame.css`
- `src/app/routing/RouterOutlet.css`
- `src/features/dashboard/Dashboard.css`
- `src/features/mods/ModLibraryPage.css`
- `src/shared/styles/layoutTokens.test.mjs`

如果实现中发现必须改动 `.tsx`，应先证明仅靠 token / CSS 无法完成目标，再单独扩展设计，不在本轮默认范围内。

## 风险与取舍

### 风险 1：3440 对部分超宽屏可能接近铺满

这是刻意接受的取舍。用户已经明确表示当前 4K 空白观感不可接受，因此本轮必须向更高利用率移动。

### 风险 2：`/mods` 增密可能让卡片显得更紧

这是本轮需要重点通过浏览器验收控制的地方。方案选择“微增密”而不是“大幅压缩”，就是为了避免从一个极端跳到另一个极端。

### 风险 3：Dashboard 视觉基线被误伤

因此本轮明确规定：

- 只让 dashboard 共享更宽的 shell
- 不给 dashboard 单独追加 4K 增密规则

## 结论

本轮 4K 修复不再讨论“是否要修”，而是已经收敛为：

- 采用 `B2`
- 把 4K 外层 shell 上限提高到约 `3440px`
- 同时让 `/mods` 在超宽档位通过更紧凑的 token 获得额外有效密度
- 保持 `dashboard` 稳定，不把它一起拉散

这是一条比当前方案明显更像桌面工具、又不会滑向激进铺满的中线修复路线。
