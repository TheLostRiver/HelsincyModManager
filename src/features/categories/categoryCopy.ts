import type { LocaleDictionary } from "../../shared/i18n";

// 分类管理页（页头、概览、工具栏、批量操作、列表行、编辑/删除、色板、排序菜单）的
// 全部用户可见文案。色板颜色值与 hex 展示为数据；mock 种子分类为内容不翻译。

export type CategoryColorKey =
  | "blue"
  | "cyan"
  | "green"
  | "amber"
  | "red"
  | "pink"
  | "purple"
  | "gray";

export type CategorySortModeKey = "custom" | "name" | "modCount";

export type CategoryCopy = {
  page: {
    eyebrow: string;
    title: string;
    metricsReady: (total: number, linkedModCount: number, emptyCategoryCount: number) => string;
    metricsLoading: string;
    metricsUnavailable: string;
    modeTabsAria: string;
    tabCategories: string;
    tabTags: string;
    createCancel: string;
    createOpen: string;
    createDialogTitle: string;
    createCloseAria: string;
    summaryAria: string;
    summaryTotal: string;
    summaryLinked: string;
    summaryEmpty: string;
    summaryColored: string;
    searchPlaceholder: string;
    searchAria: string;
    batchToggle: string;
    loadingTitle: string;
    loadingHint: string;
    errorTitle: string;
    errorHint: string;
    retry: string;
    emptyTitle: string;
    emptyHint: string;
    emptyCreate: string;
    searchEmptyTitle: string;
    searchEmptyHint: string;
    clearSearch: string;
    savingOrder: string;
    orderSaved: string;
    orderSaveFailed: string;
    created: string;
    saved: string;
    deleted: string;
  };
  sort: {
    menuAria: string;
    fallbackLabel: string;
    modes: Record<CategorySortModeKey, string>;
  };
  form: {
    formAria: string;
    nameField: string;
    namePlaceholder: string;
    colorField: string;
    newColorAria: string;
    creating: string;
    create: string;
    cancel: string;
    createFailed: string;
    duplicateName: (name: string) => string;
  };
  batch: {
    barAria: string;
    selectAllAria: string;
    selectAll: string;
    selectionSummary: (count: number, linkedModCount: number) => string;
    selectionHint: string;
    targetColorAria: string;
    applyColor: string;
    batchDelete: string;
    exitBatch: string;
    confirmDeleteLinked: (count: number, linkedModCount: number) => string;
    confirmDelete: (count: number) => string;
    deleting: string;
    delete: string;
    cancel: string;
    busy: string;
    batchDeleteLabel: string;
    batchColorLabel: string;
    batchFailed: (label: string, failedCount: number) => string;
    batchCompleted: (label: string) => string;
  };
  list: {
    listAria: string;
    headerLead: string;
    headerLinked: string;
    headerActions: string;
    selectRowAria: (name: string) => string;
    dragHint: string;
    dragDisabledHint: string;
    defaultColor: string;
    orderLabel: (order: string) => string;
    emptyCount: string;
    linkedCount: (count: number) => string;
    moveUpAria: (name: string) => string;
    moveDownAria: (name: string) => string;
    moveUp: string;
    moveDown: string;
    moveDisabledHint: string;
    editAria: (name: string) => string;
    edit: string;
    deleteAria: (name: string) => string;
    delete: string;
    editFormAria: (name: string) => string;
    nameField: string;
    colorField: string;
    editColorAria: (name: string) => string;
    saving: string;
    save: string;
    cancel: string;
    saveFailed: string;
    confirmDeleteLinked: (name: string, linkedModCount: number) => string;
    confirmDelete: (name: string) => string;
    deleting: string;
    deleteFailed: string;
  };
  colors: {
    labels: Record<CategoryColorKey, string>;
    pickAria: (label: string) => string;
    popoverAria: string;
    paletteAria: string;
    custom: string;
    customAria: string;
    clear: string;
    defaultColor: string;
  };
};

export const categoryCopy = {
  zh_cn: {
    page: {
      eyebrow: "分类 / 标签",
      title: "分类管理",
      metricsReady: (total: number, linkedModCount: number, emptyCategoryCount: number) =>
        `${total} 个分类 · 关联 ${linkedModCount} 个 Mod · ${emptyCategoryCount} 个空分类`,
      metricsLoading: "正在读取分类数据…",
      metricsUnavailable: "分类数据暂时不可用",
      modeTabsAria: "分类标签管理范围",
      tabCategories: "分类",
      tabTags: "标签",
      createCancel: "取消新建",
      createOpen: "新建分类",
      createDialogTitle: "新建分类",
      createCloseAria: "关闭新建分类",
      summaryAria: "分类概览",
      summaryTotal: "总分类",
      summaryLinked: "关联 Mod",
      summaryEmpty: "空分类",
      summaryColored: "已设置颜色",
      searchPlaceholder: "搜索分类名称…",
      searchAria: "搜索分类",
      batchToggle: "批量管理",
      loadingTitle: "正在读取分类",
      loadingHint: "请稍候",
      errorTitle: "无法加载分类列表",
      errorHint: "分类数据暂时不可用。",
      retry: "重试",
      emptyTitle: "还没有分类",
      emptyHint: "新建后可在 Mod 库和详情面板中使用。",
      emptyCreate: "新建分类",
      searchEmptyTitle: "没有匹配的分类",
      searchEmptyHint: "换个关键词，或清除搜索查看全部分类。",
      clearSearch: "清除搜索",
      savingOrder: "正在保存顺序…",
      orderSaved: "分类顺序已保存。",
      orderSaveFailed: "保存分类顺序失败，请稍后重试。",
      created: "分类已创建。",
      saved: "分类已保存。",
      deleted: "分类已删除。",
    },
    sort: {
      menuAria: "排序视图",
      fallbackLabel: "排序",
      modes: {
        custom: "自定义排序",
        name: "按名称",
        modCount: "按关联数",
      },
    },
    form: {
      formAria: "新建分类",
      nameField: "名称",
      namePlaceholder: "例如：外观",
      colorField: "颜色",
      newColorAria: "新分类颜色",
      creating: "创建中…",
      create: "创建",
      cancel: "取消",
      createFailed: "创建分类失败，请稍后重试。",
      duplicateName: (name: string) => `已存在同名分类「${name}」，请换一个名称。`,
    },
    batch: {
      barAria: "批量操作",
      selectAllAria: "全选当前列表",
      selectAll: "全选",
      selectionSummary: (count: number, linkedModCount: number) =>
        `已选 ${count} 个分类 · 关联 ${linkedModCount} 个 Mod`,
      selectionHint: "勾选分类后可批量操作",
      targetColorAria: "批量目标颜色",
      applyColor: "应用颜色",
      batchDelete: "批量删除",
      exitBatch: "退出批量",
      confirmDeleteLinked: (count: number, linkedModCount: number) =>
        `确定删除已选的 ${count} 个分类？共 ${linkedModCount} 个 Mod 关联将被移除，Mod 本体不受影响。`,
      confirmDelete: (count: number) => `确定删除已选的 ${count} 个分类？`,
      deleting: "删除中…",
      delete: "删除",
      cancel: "取消",
      busy: "正在处理批量操作…",
      batchDeleteLabel: "批量删除",
      batchColorLabel: "批量改色",
      batchFailed: (label: string, failedCount: number) =>
        `${label}有 ${failedCount} 个分类处理失败，列表已刷新。`,
      batchCompleted: (label: string) => `${label}完成。`,
    },
    list: {
      listAria: "分类列表",
      headerLead: "分类",
      headerLinked: "关联 Mod",
      headerActions: "操作",
      selectRowAria: (name: string) => `选择 ${name}`,
      dragHint: "拖拽调整顺序",
      dragDisabledHint: "在“自定义排序”视图且未搜索时可拖拽排序",
      defaultColor: "默认颜色",
      orderLabel: (order: string) => `顺序 ${order}`,
      emptyCount: "空分类",
      linkedCount: (count: number) => `${count} 个 Mod`,
      moveUpAria: (name: string) => `上移 ${name}`,
      moveDownAria: (name: string) => `下移 ${name}`,
      moveUp: "上移",
      moveDown: "下移",
      moveDisabledHint: "在“自定义排序”视图且未搜索时可调整顺序",
      editAria: (name: string) => `编辑 ${name}`,
      edit: "编辑",
      deleteAria: (name: string) => `删除 ${name}`,
      delete: "删除",
      editFormAria: (name: string) => `编辑 ${name}`,
      nameField: "名称",
      colorField: "颜色",
      editColorAria: (name: string) => `编辑 ${name} 的颜色`,
      saving: "保存中…",
      save: "保存",
      cancel: "取消",
      saveFailed: "保存分类失败，请稍后重试。",
      confirmDeleteLinked: (name: string, linkedModCount: number) =>
        `确定删除「${name}」？有 ${linkedModCount} 个 Mod 关联将被移除，Mod 本体不受影响。`,
      confirmDelete: (name: string) => `确定删除「${name}」？`,
      deleting: "删除中…",
      deleteFailed: "删除分类失败，请稍后重试。",
    },
    colors: {
      labels: {
        blue: "蓝色",
        cyan: "青色",
        green: "绿色",
        amber: "琥珀",
        red: "红色",
        pink: "粉色",
        purple: "紫色",
        gray: "灰色",
      },
      pickAria: (label: string) => `选择${label}`,
      popoverAria: "选择分类颜色",
      paletteAria: "常用颜色",
      custom: "自定义",
      customAria: "自定义颜色",
      clear: "恢复默认颜色",
      defaultColor: "默认颜色",
    },
  },
  en: {
    page: {
      eyebrow: "Categories / Tags",
      title: "Category management",
      metricsReady: (total: number, linkedModCount: number, emptyCategoryCount: number) =>
        `${total} categories · ${linkedModCount} linked mod(s) · ${emptyCategoryCount} empty`,
      metricsLoading: "Loading category data…",
      metricsUnavailable: "Category data is temporarily unavailable",
      modeTabsAria: "Category and tag management scope",
      tabCategories: "Categories",
      tabTags: "Tags",
      createCancel: "Cancel creation",
      createOpen: "New category",
      createDialogTitle: "New category",
      createCloseAria: "Close new category form",
      summaryAria: "Category overview",
      summaryTotal: "Total",
      summaryLinked: "Linked mods",
      summaryEmpty: "Empty",
      summaryColored: "Colored",
      searchPlaceholder: "Search category names…",
      searchAria: "Search categories",
      batchToggle: "Batch manage",
      loadingTitle: "Loading categories",
      loadingHint: "Please wait",
      errorTitle: "Failed to load categories",
      errorHint: "Category data is temporarily unavailable.",
      retry: "Retry",
      emptyTitle: "No categories yet",
      emptyHint: "After creating one, it becomes usable in the mod library and detail panels.",
      emptyCreate: "New category",
      searchEmptyTitle: "No matching categories",
      searchEmptyHint: "Try another keyword, or clear the search to see all categories.",
      clearSearch: "Clear search",
      savingOrder: "Saving order…",
      orderSaved: "Category order saved.",
      orderSaveFailed: "Saving the category order failed. Please try again later.",
      created: "Category created.",
      saved: "Category saved.",
      deleted: "Category deleted.",
    },
    sort: {
      menuAria: "Sort view",
      fallbackLabel: "Sort",
      modes: {
        custom: "Custom order",
        name: "By name",
        modCount: "By linked count",
      },
    },
    form: {
      formAria: "New category",
      nameField: "Name",
      namePlaceholder: "e.g. Appearance",
      colorField: "Color",
      newColorAria: "New category color",
      creating: "Creating…",
      create: "Create",
      cancel: "Cancel",
      createFailed: "Creating the category failed. Please try again later.",
      duplicateName: (name: string) => `A category named "${name}" already exists. Choose another name.`,
    },
    batch: {
      barAria: "Batch actions",
      selectAllAria: "Select all in the current list",
      selectAll: "Select all",
      selectionSummary: (count: number, linkedModCount: number) =>
        `${count} selected · ${linkedModCount} linked mod(s)`,
      selectionHint: "Check categories to run batch actions",
      targetColorAria: "Batch target color",
      applyColor: "Apply color",
      batchDelete: "Batch delete",
      exitBatch: "Exit batch",
      confirmDeleteLinked: (count: number, linkedModCount: number) =>
        `Delete the ${count} selected categories? ${linkedModCount} mod link(s) will be removed; the mods themselves are unaffected.`,
      confirmDelete: (count: number) => `Delete the ${count} selected categories?`,
      deleting: "Deleting…",
      delete: "Delete",
      cancel: "Cancel",
      busy: "Processing batch actions…",
      batchDeleteLabel: "Batch delete",
      batchColorLabel: "Batch recolor",
      batchFailed: (label: string, failedCount: number) =>
        `${label}: ${failedCount} categories failed; the list has been refreshed.`,
      batchCompleted: (label: string) => `${label} completed.`,
    },
    list: {
      listAria: "Category list",
      headerLead: "Category",
      headerLinked: "Linked mods",
      headerActions: "Actions",
      selectRowAria: (name: string) => `Select ${name}`,
      dragHint: "Drag to reorder",
      dragDisabledHint: "Reordering works in the \"Custom order\" view without an active search",
      defaultColor: "Default color",
      orderLabel: (order: string) => `Order ${order}`,
      emptyCount: "Empty",
      linkedCount: (count: number) => `${count} mod(s)`,
      moveUpAria: (name: string) => `Move ${name} up`,
      moveDownAria: (name: string) => `Move ${name} down`,
      moveUp: "Move up",
      moveDown: "Move down",
      moveDisabledHint: "Reordering works in the \"Custom order\" view without an active search",
      editAria: (name: string) => `Edit ${name}`,
      edit: "Edit",
      deleteAria: (name: string) => `Delete ${name}`,
      delete: "Delete",
      editFormAria: (name: string) => `Edit ${name}`,
      nameField: "Name",
      colorField: "Color",
      editColorAria: (name: string) => `Edit the color of ${name}`,
      saving: "Saving…",
      save: "Save",
      cancel: "Cancel",
      saveFailed: "Saving the category failed. Please try again later.",
      confirmDeleteLinked: (name: string, linkedModCount: number) =>
        `Delete "${name}"? ${linkedModCount} mod link(s) will be removed; the mods themselves are unaffected.`,
      confirmDelete: (name: string) => `Delete "${name}"?`,
      deleting: "Deleting…",
      deleteFailed: "Deleting the category failed. Please try again later.",
    },
    colors: {
      labels: {
        blue: "Blue",
        cyan: "Cyan",
        green: "Green",
        amber: "Amber",
        red: "Red",
        pink: "Pink",
        purple: "Purple",
        gray: "Gray",
      },
      pickAria: (label: string) => `Pick ${label}`,
      popoverAria: "Pick a category color",
      paletteAria: "Common colors",
      custom: "Custom",
      customAria: "Custom color",
      clear: "Restore default color",
      defaultColor: "Default color",
    },
  },
  ja: {
    page: {
      eyebrow: "カテゴリ / タグ",
      title: "カテゴリ管理",
      metricsReady: (total: number, linkedModCount: number, emptyCategoryCount: number) =>
        `カテゴリ ${total} 件 · 関連 Mod ${linkedModCount} 件 · 空カテゴリ ${emptyCategoryCount} 件`,
      metricsLoading: "カテゴリデータを読み込み中…",
      metricsUnavailable: "カテゴリデータを一時的に利用できません",
      modeTabsAria: "カテゴリ・タグ管理の範囲",
      tabCategories: "カテゴリ",
      tabTags: "タグ",
      createCancel: "作成をやめる",
      createOpen: "カテゴリを新規作成",
      createDialogTitle: "カテゴリを新規作成",
      createCloseAria: "新規作成フォームを閉じる",
      summaryAria: "カテゴリ概要",
      summaryTotal: "総数",
      summaryLinked: "関連 Mod",
      summaryEmpty: "空カテゴリ",
      summaryColored: "色設定済み",
      searchPlaceholder: "カテゴリ名を検索…",
      searchAria: "カテゴリを検索",
      batchToggle: "一括管理",
      loadingTitle: "カテゴリを読み込み中",
      loadingHint: "お待ちください",
      errorTitle: "カテゴリ一覧を読み込めません",
      errorHint: "カテゴリデータを一時的に利用できません。",
      retry: "再試行",
      emptyTitle: "カテゴリはまだありません",
      emptyHint: "作成すると Mod ライブラリと詳細パネルで使用できます。",
      emptyCreate: "カテゴリを新規作成",
      searchEmptyTitle: "一致するカテゴリがありません",
      searchEmptyHint: "別のキーワードを試すか、検索をクリアして全カテゴリを表示してください。",
      clearSearch: "検索をクリア",
      savingOrder: "順序を保存中…",
      orderSaved: "カテゴリの順序を保存しました。",
      orderSaveFailed: "カテゴリ順序の保存に失敗しました。しばらくしてから再試行してください。",
      created: "カテゴリを作成しました。",
      saved: "カテゴリを保存しました。",
      deleted: "カテゴリを削除しました。",
    },
    sort: {
      menuAria: "並び替えビュー",
      fallbackLabel: "並び替え",
      modes: {
        custom: "カスタム順",
        name: "名前順",
        modCount: "関連数順",
      },
    },
    form: {
      formAria: "カテゴリを新規作成",
      nameField: "名前",
      namePlaceholder: "例：外見",
      colorField: "色",
      newColorAria: "新しいカテゴリの色",
      creating: "作成中…",
      create: "作成",
      cancel: "キャンセル",
      createFailed: "カテゴリの作成に失敗しました。しばらくしてから再試行してください。",
      duplicateName: (name: string) => `同名のカテゴリ「${name}」が既に存在します。別の名前にしてください。`,
    },
    batch: {
      barAria: "一括操作",
      selectAllAria: "現在の一覧をすべて選択",
      selectAll: "すべて選択",
      selectionSummary: (count: number, linkedModCount: number) =>
        `${count} 件選択 · 関連 Mod ${linkedModCount} 件`,
      selectionHint: "カテゴリを選択すると一括操作できます",
      targetColorAria: "一括適用する色",
      applyColor: "色を適用",
      batchDelete: "一括削除",
      exitBatch: "一括を終了",
      confirmDeleteLinked: (count: number, linkedModCount: number) =>
        `選択した ${count} 件のカテゴリを削除しますか？計 ${linkedModCount} 件の Mod 関連付けが解除されます。Mod 本体には影響しません。`,
      confirmDelete: (count: number) => `選択した ${count} 件のカテゴリを削除しますか？`,
      deleting: "削除中…",
      delete: "削除",
      cancel: "キャンセル",
      busy: "一括操作を処理中…",
      batchDeleteLabel: "一括削除",
      batchColorLabel: "一括色変更",
      batchFailed: (label: string, failedCount: number) =>
        `${label}で ${failedCount} 件のカテゴリの処理に失敗しました。一覧は更新済みです。`,
      batchCompleted: (label: string) => `${label}が完了しました。`,
    },
    list: {
      listAria: "カテゴリ一覧",
      headerLead: "カテゴリ",
      headerLinked: "関連 Mod",
      headerActions: "操作",
      selectRowAria: (name: string) => `${name} を選択`,
      dragHint: "ドラッグで並び替え",
      dragDisabledHint: "「カスタム順」ビューで検索していないときに並び替えできます",
      defaultColor: "既定の色",
      orderLabel: (order: string) => `順序 ${order}`,
      emptyCount: "空カテゴリ",
      linkedCount: (count: number) => `Mod ${count} 件`,
      moveUpAria: (name: string) => `${name} を上へ移動`,
      moveDownAria: (name: string) => `${name} を下へ移動`,
      moveUp: "上へ",
      moveDown: "下へ",
      moveDisabledHint: "「カスタム順」ビューで検索していないときに順序を変更できます",
      editAria: (name: string) => `${name} を編集`,
      edit: "編集",
      deleteAria: (name: string) => `${name} を削除`,
      delete: "削除",
      editFormAria: (name: string) => `${name} を編集`,
      nameField: "名前",
      colorField: "色",
      editColorAria: (name: string) => `${name} の色を編集`,
      saving: "保存中…",
      save: "保存",
      cancel: "キャンセル",
      saveFailed: "カテゴリの保存に失敗しました。しばらくしてから再試行してください。",
      confirmDeleteLinked: (name: string, linkedModCount: number) =>
        `「${name}」を削除しますか？${linkedModCount} 件の Mod 関連付けが解除されます。Mod 本体には影響しません。`,
      confirmDelete: (name: string) => `「${name}」を削除しますか？`,
      deleting: "削除中…",
      deleteFailed: "カテゴリの削除に失敗しました。しばらくしてから再試行してください。",
    },
    colors: {
      labels: {
        blue: "青",
        cyan: "シアン",
        green: "緑",
        amber: "アンバー",
        red: "赤",
        pink: "ピンク",
        purple: "紫",
        gray: "グレー",
      },
      pickAria: (label: string) => `${label}を選択`,
      popoverAria: "カテゴリの色を選択",
      paletteAria: "よく使う色",
      custom: "カスタム",
      customAria: "カスタムカラー",
      clear: "既定の色に戻す",
      defaultColor: "既定の色",
    },
  },
} satisfies LocaleDictionary<CategoryCopy>;
