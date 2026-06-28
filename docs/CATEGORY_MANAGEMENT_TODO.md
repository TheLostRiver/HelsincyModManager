# 分类标签管理 待办

本文档维护分类标签功能的后续切片和已完成基线。它不是一次性 PR 计划，而是分类能力继续推进时的任务入口。

最近同步：2026-06-28，基于 `598f45e` (main)。

架构约束以 [架构文档](ARCHITECTURE.md) 为准；前后端通信形状参考 [前后端通信契约](FRONTEND_BACKEND_CONTRACT.md)；外观规范参考 [外观系统](APPEARANCE_SYSTEM.md)。

## 目标

分类标签功能的完整交付链路：

```text
全局分类 CRUD
  -> 用户在管理页面创建/编辑/删除分类
  -> 用户在 Mod 详情关联分类
  -> Mod 库筛选栏动态展示分类 chips
  -> 用户分类与导入标签合并显示
```

设计约束：

- 分类与 Mod 是多对多关系，通过 `mod_categories` 中间表维护。
- 关联语义为全量替换：`set_mod_categories(modId, [id1, id2])` 替换该 Mod 全部关联。
- 用户分类（有 color）排在导入标签（无 color）之前，按 name 精确匹配去重。
- 删除分类时 CASCADE 自动清除关联，前端删除确认时展示关联 Mod 数量。
- category_id 由后端生成（UUID v4），前端只传 name/color/sortOrder。
- 排序规则：`sort_order ASC, name ASC`。

## 当前基线

已落地（T4 — PR #112）：

- `hmm-core`：`Category` 领域模型（id, name, color, sort_order, created_at）、`CategoryLabel` 轻量视图模型。
- `hmm-ports`：`CategoryRepository` trait（8 方法，含 `list_mod_category_pairs` 批量加载）。
- `hmm-infra`：`SqliteCategoryRepository`（UPSERT + 事务性关联替换 + CASCADE），20+ 单元测试。
- `hmm-app`：`CategoryService`（CRUD + 输入归一化 + UUID 生成 + dedup），`category_labels_from_metadata`、`build_user_category_map`、`merge_category_labels` 合并逻辑。
- `hmm-app`：`ModLibraryService.get_mod_library()` 合并用户分类与导入标签（用户在前、精确去重）。
- `hmm-tauri`：6 个 Tauri commands（create/update/delete_category、list_categories、set/get_mod_categories）+ DTO。
- 前端 typed API：`modCategoryApi.ts`（6 函数 + 4 类型）。
- `ModLibraryItem.categoryLabels` 从 `string[]` 升级为 `CategoryLabel[]`。
- `ModLibraryPage` 筛选逻辑适配 `CategoryLabel` 结构。
- SQLite migration 001 已创建 `categories`、`mod_categories` 表。

已落地（分类管理页面，PR #113，review follow-up PR #114）：

- `/categories` 路由注册 + 侧边栏自动启用。
- 分类管理页面：查看列表、新建、行内编辑、删除（含 modCount 警告）。
- 空状态、摘要卡片、颜色预览面板、亮/暗主题适配。
- 新建/编辑分类颜色使用悬浮色板 UI，不再要求用户手填 hex code。
- 分类 CRUD 失败展示行内错误，且前端不直接展示后端 raw error message。
- 编辑态分类行保留 `role="listitem"`，色板按钮提供 `aria-pressed`。
- 删除确认布局已有测试覆盖。

> 命名说明：本文档中的“分类管理页面”曾作为分类专题的 T5 切片推进；根目录 `TODO.md` 中的 T5 仍指“Mod 信息编辑面板”，二者不是同一个交付物。

## 已完成切片：分类管理页面

### 已完成范围

- `/categories` 路由注册 + 侧边栏自动启用。
- 分类管理页面：查看列表、新建、行内编辑、删除（含 modCount 警告）。
- 空状态引导。
- 亮/暗主题适配。
- 悬浮色板颜色选择器。
- 基础可访问性与 review follow-up 修复。

### 仍不包含的事

- Mod-分类关联 UI（Mod 详情面板打标签）。
- ModLibraryPage 动态筛选 chips（替换硬编码 `libraryFilterChips`）。
- 拖拽排序（用数字 input 调整 sortOrder）。
- 批量操作。
- 后端改动。

## 后续切片

以下能力尚未开始，后续按优先级分别切 PR：

- [ ] Mod 详情面板分类关联 UI：在 Mod 详情展示已关联分类标签，提供添加/移除操作，调用 `setModCategories`。
- [ ] ModLibraryPage 动态筛选 chips：从 `listCategories()` 查询替换硬编码 `libraryFilterChips`，按 sortOrder 排列，空分类不显示 chip。
- [ ] ModPosterCard 分类标签渲染：在卡片上展示分类色块/名称。
- [ ] 默认分类种子数据：首次启动或首次导入时自动创建默认分类（外观、武器替换、语音替换、功能性 Mod、前置、工具）。
- [ ] 分类拖拽排序：引入拖拽库替代数字 input 调整 sortOrder。
- [ ] 分类批量操作：批量删除、批量改色。

## 已完成切片记录

- [x] T4：后端分类 CRUD、typed API、CategoryLabel 升级、合并逻辑、6 Tauri commands（PR #112）。
- [x] 分类管理页面：`/categories` 页面、新建/编辑/删除、悬浮色板、行内错误与 review follow-up（PR #113 / #114）。
