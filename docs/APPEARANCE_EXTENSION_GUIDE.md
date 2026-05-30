# 前端外观系统扩展指南

本文档是 [前端外观系统设计](APPEARANCE_SYSTEM.md) 的配套扩展说明。主设计文档回答“系统是什么、边界在哪里”，本文回答“以后新增外观能力时具体怎么做、怎么验收、怎么避免越界”。

外观系统的扩展目标不是让项目拥有很多花哨皮肤，而是让 Helsincy 在长期迭代中可以稳定支持不同玩家偏好、不同屏幕尺寸和不同工作流，同时不让业务页面、游戏适配器和全局样式互相缠住。

## 适用范围

本文适用于以下改动：

- 新增颜色方案，例如深色、猎人绿、冰原冷色。
- 新增 Shell 变体，例如浮动 Dock、紧凑 Rail、顶部导航。
- 新增外观预设，例如“默认浅色”“冰原专注”“紧凑工作台”。
- 新增密度等级，例如舒适、紧凑、高信息密度。
- 新增动效等级，例如关闭、轻动效、正常动效。
- 调整导航在不同 Shell 下的呈现方式。
- 在设置页中接入外观选择。
- 将现有页面迁移到语义 token。

不适用于以下改动：

- Mod 安装、卸载、备份、回滚流程。
- 游戏 adapter 规则。
- 替换目标 catalog。
- Tauri command 文件系统能力。
- 远程主题市场、第三方主题导入或插件系统。

如果一次改动同时涉及外观系统和真实文件写入，应拆成两个 PR。外观系统不应顺手修改高风险业务路径。

## 扩展原则

### 1. 先选维度，再写实现

新增外观能力前，先判断它属于哪个维度：

| 需求 | 所属维度 | 示例 |
|------|----------|------|
| 改颜色、对比度、语义色 | `colorScheme` | 深色、冰原、猎人绿 |
| 改导航和全局布局 | `shellVariant` | 浮动 Dock、紧凑 Rail |
| 改控件高度和间距 | `density` | 舒适、紧凑 |
| 改动画速度和过渡 | `motion` | 无动效、轻动效 |
| 组合多个维度供玩家选择 | `appearancePreset` | 冰原专注 |

不要把不同维度混在一个实现里。例如“冰原主题”如果既想改颜色又想用紧凑 Rail，应建一个 preset 组合 `colorScheme: "iceborne"` 和 `shellVariant: "compact-rail"`，而不是让 `iceborne.css` 控制导航结构。

### 2. Shell 变体只管应用外壳

Shell 可以决定：

- 主导航放左侧、底部还是浮动层。
- 顶部状态栏如何排列。
- 主内容区域是否需要为浮动控件留安全距离。
- 导航项折叠、展开、tooltip 如何呈现。

Shell 不能决定：

- Mod 是否可安装。
- 游戏目录是否有效。
- 任务是否失败。
- 替换目标如何映射。
- 某个游戏是否走特殊文件路径。

这些判断应来自应用状态、用例结果或游戏 adapter。Shell 只消费摘要，不参与业务推理。

### 3. 业务页面只依赖语义 token

业务页面可以使用：

```css
.panel {
  color: var(--color-text);
  background: var(--color-surface);
  border: 1px solid var(--color-border);
  padding: var(--space-panel);
}
```

业务页面不应写：

```css
.panel {
  background: #ffffff;
}

[data-shell-variant="floating-dock"] .mods-page {
  margin-left: 96px;
}
```

如果业务页面需要适配可用空间，应使用正常响应式布局、容器宽度或组件自身的变体，而不是读取当前 Shell 名称。

### 4. 扩展必须可回退

任何新增外观配置都必须有安全回退：

- 未知 `presetId` 回退默认预设。
- 未知 `colorScheme` 回退 `light` 或系统默认。
- 未知 `shellVariant` 回退 `classic-sidebar`。
- 未知 `density` 回退 `comfortable`。
- 未知 `motion` 回退 `subtle` 或 `none`。

外观设置损坏不应导致白屏。

## 命名规范

### ID

外观相关 ID 使用小写 kebab-case：

```text
light
dark
iceborne
hunter-green
classic-sidebar
floating-dock
compact-rail
default-light
iceborne-focus
```

不要使用中文、空格、大小写混合或视觉描述过长的 ID。

### 文件名

组件文件使用 PascalCase：

```text
FloatingDockShell.tsx
CompactRailShell.tsx
NavigationButton.tsx
```

样式文件与组件同名或按维度命名：

```text
FloatingDockShell.css
iceborne.css
compact.css
subtle.css
```

注册表和类型文件使用清晰英文名：

```text
appearanceRegistry.ts
appearanceTypes.ts
shellRegistry.ts
navItems.ts
```

### CSS variables

CSS variables 使用语义名称，不使用主题名称：

```css
--color-surface
--color-surface-muted
--color-text
--color-text-muted
--color-border
--color-accent
--space-panel
--size-nav-item
--motion-duration-fast
```

禁止：

```css
--iceborne-blue
--dock-left-width
--mhw-panel-bg
```

如果某个变量只对 Shell 内部有意义，应放在 Shell 样式内部并加局部前缀，例如：

```css
.floating-dock-shell {
  --floating-dock-width: 56px;
}
```

## 推荐文件边界

```text
src/app/appearance/
  appearanceTypes.ts       # 类型定义，不放 React 渲染
  appearanceRegistry.ts    # 预设、颜色、密度、动效注册表
  AppearanceProvider.tsx   # 运行时状态和 data-* 应用
  useAppearance.ts         # hook 出口
  persistAppearance.ts     # 设置读写与迁移

src/app/shell/
  AppShell.tsx             # 选择当前 Shell，不写具体布局细节
  shellRegistry.ts         # Shell 变体注册
  layouts/*                # 每个 Shell 变体独立目录
  navigation/*             # 共享导航定义和基础导航组件

src/shared/styles/
  tokens.css               # 基础语义 token
  color-schemes/*.css      # 颜色方案
  density/*.css            # 密度 token
  motion/*.css             # 动效 token
```

一个文件只能承担一个主要职责。如果某个 Shell 文件开始同时处理导航定义、状态推导、响应式布局和设置读写，就应该拆分。

## 新增颜色方案

### 适用场景

当需求主要是改变视觉色彩、对比度或语义色时，新增颜色方案。

适合：

- 深色模式。
- 冰原冷色。
- 猎人绿。
- 高对比度主题。

不适合：

- 改侧边栏为浮动 Dock。
- 改导航项尺寸。
- 改页面信息密度。

### 文件改动

推荐改动：

```text
src/shared/styles/color-schemes/<id>.css
src/app/appearance/appearanceRegistry.ts
src/app/appearance/appearanceTypes.ts
```

如果 `ColorSchemeId` 是字符串联合类型，需要加入新 ID：

```ts
export type ColorSchemeId = "light" | "dark" | "iceborne" | "hunter-green";
```

注册颜色方案：

```ts
export const colorSchemes: ColorSchemeDefinition[] = [
  { id: "light", name: "浅色" },
  { id: "dark", name: "深色" },
  { id: "iceborne", name: "冰原" },
];
```

新增 CSS：

```css
:root[data-color-scheme="iceborne"] {
  --color-bg: #f3f8fb;
  --color-surface: #ffffff;
  --color-surface-muted: #eaf3f8;
  --color-border: #cbdde8;
  --color-text: #17232f;
  --color-text-muted: #5a6b7a;
  --color-accent: #2f7ea8;
  --color-danger: #c2413b;
  --color-warning: #b7791f;
}
```

### 验收清单

- 所有核心语义色都有值。
- 普通文本、弱文本、边框、警告、危险状态可读。
- 业务页面没有新增硬编码色值。
- 深浅背景下 icon 和按钮状态清楚。
- `git diff` 中没有把布局逻辑写进颜色 CSS。

## 新增 Shell 变体

### 适用场景

当需求会改变全局导航、状态栏或页面外壳布局时，新增 Shell 变体。

适合：

- 左侧经典侧边栏。
- 浮动 Dock。
- 紧凑 Rail。
- 底部导航。

不适合：

- 单个页面内部换布局。
- 某个按钮换颜色。
- Mod 列表从卡片变表格。

### 文件改动

推荐新增：

```text
src/app/shell/layouts/<variant-id>/<VariantName>Shell.tsx
src/app/shell/layouts/<variant-id>/<VariantName>Shell.css
```

推荐修改：

```text
src/app/shell/shellRegistry.ts
src/app/appearance/appearanceTypes.ts
src/app/appearance/appearanceRegistry.ts
```

Shell 类型示例：

```ts
export type ShellVariantId = "classic-sidebar" | "floating-dock" | "compact-rail";
```

注册 Shell：

```ts
export const shellRegistry: Record<ShellVariantId, ShellDefinition> = {
  "classic-sidebar": {
    id: "classic-sidebar",
    name: "经典侧边栏",
    component: ClassicSidebarShell,
  },
  "floating-dock": {
    id: "floating-dock",
    name: "浮动 Dock",
    component: FloatingDockShell,
  },
  "compact-rail": {
    id: "compact-rail",
    name: "紧凑 Rail",
    component: CompactRailShell,
  },
};
```

Shell 组件骨架：

```tsx
export function FloatingDockShell({ navItems, status, children }: AppShellLayoutProps) {
  return (
    <div className="floating-dock-shell">
      <TopStatusBar status={status} />
      <FloatingDockNavigation navItems={navItems} />
      <main className="floating-dock-shell__main">{children}</main>
    </div>
  );
}
```

### 浮动 Dock 特别要求

浮动 Dock 必须额外检查：

- 折叠态图标有 `aria-label` 或 tooltip。
- 展开态文字不会挤压主内容。
- Dock 层级低于模态框和危险确认弹窗。
- 小屏下不会遮挡底部操作区。
- 当前页面在折叠态仍可识别。
- 键盘用户可以进入、切换、离开 Dock。

建议 CSS 结构：

```css
.floating-dock-shell {
  min-height: 100vh;
  color: var(--color-text);
  background: var(--color-bg);
}

.floating-dock-shell__main {
  min-width: 0;
  padding: var(--space-page);
}

.floating-dock {
  position: fixed;
  z-index: var(--z-navigation);
}
```

不要让业务页面通过 margin 适配 Dock。应由 Shell 自己管理安全区域。

### 验收清单

- 使用同一份 `navItems`。
- 使用同一份 `AppStatusSummary`。
- 主内容区域存在 `main`。
- 不调用 Tauri command。
- 不复制业务页面。
- 不读取游戏 adapter。
- 在桌面、窄屏和近似 Steam Deck 分辨率下可用。

## 新增密度等级

### 适用场景

当需求主要是让界面更宽松或更紧凑时，新增密度等级。

密度应影响：

- 控件高度。
- 列表行高。
- 面板内边距。
- 导航项高度。
- 表格单元格间距。

密度不应影响：

- 颜色。
- 业务字段是否存在。
- 游戏规则。
- Shell 结构。

### 文件改动

```text
src/shared/styles/density/<id>.css
src/app/appearance/appearanceTypes.ts
src/app/appearance/appearanceRegistry.ts
```

示例：

```css
:root[data-density="compact"] {
  --space-page: 16px;
  --space-panel: 14px;
  --space-control-x: 10px;
  --space-control-y: 7px;
  --size-nav-item: 30px;
  --size-table-row: 36px;
}
```

### 验收清单

- 文本不被截断。
- 按钮仍可点击。
- 表格行内容不重叠。
- 小屏下不会因为紧凑模式导致交互目标过小。
- 没有在业务页面中写 `if density === "compact"`。

## 新增动效等级

### 适用场景

当需求主要是改变过渡速度、展开收起动画或降低动画刺激时，新增动效等级。

动效 token 示例：

```css
:root[data-motion="normal"] {
  --motion-duration-fast: 140ms;
  --motion-duration-normal: 220ms;
  --motion-easing-standard: cubic-bezier(0.2, 0, 0, 1);
}

@media (prefers-reduced-motion: reduce) {
  :root {
    --motion-duration-fast: 0ms;
    --motion-duration-normal: 0ms;
  }
}
```

### 验收清单

- 尊重 `prefers-reduced-motion`。
- 关键操作不依赖动画结束才能使用。
- 关闭动效时没有视觉残留。
- 浮动 Dock 展开收起不会导致主内容跳动。

## 新增外观预设

外观预设是多个维度的组合，不应写新的组件。

示例：

```ts
export const appearancePresets: AppearancePreset[] = [
  {
    id: "default-light",
    name: "默认浅色",
    colorScheme: "light",
    shellVariant: "classic-sidebar",
    density: "comfortable",
    motion: "subtle",
  },
  {
    id: "iceborne-focus",
    name: "冰原专注",
    colorScheme: "iceborne",
    shellVariant: "compact-rail",
    density: "compact",
    motion: "subtle",
  },
];
```

新增预设前先确认：

- 组合中的每个 ID 都已经注册。
- 名称能让玩家理解差异。
- 不是为了绕过缺失的底层能力。
- 没有把游戏规则塞进预设。

## 设置页接入

设置页应提供两层能力：

- 快速选择外观预设。
- 高级模式下分别选择颜色、Shell、密度、动效。

推荐交互：

```text
外观预设
  默认浅色
  默认深色
  冰原专注

高级设置
  颜色方案
  导航布局
  信息密度
  动效偏好
```

设置页只修改 `AppearanceSettings`，不直接操作 DOM。DOM 的 `data-*` 属性由 `AppearanceProvider` 统一应用。

错误处理：

- 设置保存失败时回退内存中的当前设置。
- 设置读取失败时使用默认预设，并展示可恢复提示。
- 未知配置值进入迁移逻辑，不直接抛出白屏错误。

## 持久化与迁移

推荐设置结构：

```ts
type PersistedAppearanceSettings = {
  version: 1;
  presetId: string;
  overrides?: Partial<{
    colorScheme: ColorSchemeId;
    shellVariant: ShellVariantId;
    density: DensityId;
    motion: MotionId;
  }>;
};
```

迁移规则：

- 新增字段必须有默认值。
- 删除预设时必须提供替代映射。
- 重命名 ID 时必须保留迁移表。
- 读取失败时不写坏原始配置，先回退运行时默认值。

示例：

```ts
const presetMigration: Record<string, string> = {
  "ice-blue": "iceborne-focus",
};
```

## 与路由和导航的关系

导航定义可以包含 route 和 capability，但不应包含布局细节。

允许：

```ts
{
  id: "backups",
  label: "存档备份",
  route: "/backups",
  capability: "save-backup",
}
```

禁止：

```ts
{
  id: "backups",
  label: "存档备份",
  route: "/backups",
  dockPosition: "bottom",
  onlyForShell: "floating-dock",
}
```

如果某个 Shell 需要特殊分组，应在 Shell 内部基于通用字段做呈现，而不是污染导航定义。例如可以把设置、日志放到 Dock 底部，但这个规则属于 `FloatingDockNavigation` 的呈现逻辑。

## 与功能页面的关系

功能页面不应知道自己运行在哪个 Shell 中。

允许：

- 页面根据容器宽度自适应。
- 页面使用通用布局组件。
- 页面读取当前游戏 capability 来决定功能是否可用。

禁止：

- 页面判断 `shellVariant`。
- 页面为每个 Shell 复制一份。
- 页面直接修改 `document.dataset`。
- 页面直接写全局 CSS 变量。

如果页面确实需要不同密度下展示不同信息量，优先通过 CSS 和组件 props 解决；只有当产品语义真的不同，才考虑页面级配置，但不能绑定具体 Shell 名称。

## 安全与隐私

外观扩展不能降低桌面应用安全边界。

必须遵守：

- 不允许用户输入任意 CSS。
- 不允许用户输入任意 HTML。
- 不允许主题执行 JS。
- 不允许主题引用本地图片路径。
- 不允许主题远程拉取字体或背景。
- 不记录完整本地路径、Steam ID、token、cookie。
- 外观配置只保存外观相关 ID 和受控 override。

未来如果支持主题包导入，必须另写安全设计，至少覆盖：

- 包格式。
- 文件大小限制。
- magic bytes 校验。
- 路径穿越防御。
- 允许字段白名单。
- CSP 影响。
- 诊断日志脱敏。

## 可访问性与响应式矩阵

每个新增 Shell 或主题至少检查：

| 场景 | 检查重点 |
|------|----------|
| 键盘导航 | Tab 顺序、当前项、焦点环 |
| 屏幕阅读器 | `aria-label`、`aria-current`、状态文本 |
| 减少动效 | 动画关闭后仍可理解 |
| 高对比需求 | 文本、边框、警告、危险状态可读 |
| 860px 以下 | 导航不丢失、内容不重叠 |
| Steam Deck 近似分辨率 | 触控目标、紧凑布局、状态栏可见 |

视觉上“好看”不能替代可访问性验收。

## 测试建议

实现阶段建议按层次测试：

### 单元测试

- registry 是否包含默认项。
- 未知 ID 是否回退。
- 设置迁移是否稳定。
- preset 是否引用已存在维度。

### 组件测试

- `AppShell` 能根据 appearance 选择 Shell。
- 每个 Shell 能渲染相同导航项。
- 禁用导航项有可解释状态。
- 当前页面状态正确标记。

### 视觉和手动 smoke test

- 切换颜色方案。
- 切换 Shell 变体。
- 切换密度。
- 切换动效。
- 重启应用后设置仍然生效。
- 配置损坏时回退默认外观。

当前项目的统一验证入口仍然是：

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify.ps1
```

## PR 检查清单

提交外观扩展 PR 前，应确认：

- [ ] 改动属于正确维度。
- [ ] 新 ID 使用 kebab-case。
- [ ] 新增项已经注册。
- [ ] 有默认回退策略。
- [ ] 没有业务页面判断 `shellVariant`。
- [ ] 没有新增任意 CSS / HTML / JS 注入入口。
- [ ] 没有把游戏规则写进外观系统。
- [ ] 没有复制整套业务页面。
- [ ] 文档入口和 CHANGELOG 已更新。
- [ ] 已执行必要验证，并在 PR 中记录结果。

## 推荐扩展顺序

建议按以下顺序落地：

1. 抽离 `navItems` 和 `classic-sidebar`，先不改变视觉。
2. 引入 `AppearanceProvider`，支持默认预设和回退。
3. 抽出颜色 token，支持 `light` / `dark`。
4. 抽出密度和动效 token。
5. 新增 `compact-rail`，验证 Shell 可替换。
6. 新增 `floating-dock`，验证复杂 Shell 不影响业务页面。
7. 在设置页接入外观预设和高级选项。

这个顺序能让每一步都可测试、可回退，也能避免一开始就把外观系统做成大而难改的总控模块。

## 常见错误

### 错误：把主题当成页面分支

```tsx
if (appearance.colorScheme === "iceborne") {
  return <IceborneDashboard />;
}
```

应改为让页面使用语义 token，颜色方案只提供变量。

### 错误：Shell 读取业务实现

```tsx
const mods = await invoke("list_mods");
```

Shell 不应调用 Tauri command。业务数据由页面或上层应用状态提供。

### 错误：Dock 影响业务页面布局

```css
[data-shell-variant="floating-dock"] .mods-page {
  padding-left: 96px;
}
```

应由 `FloatingDockShell` 管理主内容安全区域。

### 错误：复制导航定义

```text
ClassicSidebarNavItems.ts
FloatingDockNavItems.ts
CompactRailNavItems.ts
```

应保留一份 `navItems.ts`，不同 Shell 只决定呈现。

### 错误：让主题配置承载游戏规则

```ts
{
  colorScheme: "iceborne",
  requiredDependency: "strackers-loader"
}
```

前置依赖属于游戏 adapter 或依赖规则 catalog，不属于外观系统。

## 结论

外观系统扩展的关键不是“多加几个主题”，而是让每一种变化都有正确位置：

- 颜色进 `colorScheme`。
- 全局布局进 `shellVariant`。
- 间距尺寸进 `density`。
- 动画节奏进 `motion`。
- 玩家可选组合进 `appearancePreset`。
- 业务规则留在业务层和游戏 adapter。

只要这条边界守住，Helsincy 后续就可以大胆尝试浮动 Dock、冰原风格、紧凑工作台和 Steam Deck 友好布局，而不会把前端重新拖进一个巨大的 `AppShell` 文件里。
