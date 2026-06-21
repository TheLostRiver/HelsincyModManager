type ScrollTarget = {
  scrollTo: (options: ScrollToOptions) => void;
};

type QueryDocument = {
  querySelector: (selector: string) => ScrollTarget | null;
};

export function getModLibraryBackToTopTarget(documentLike: QueryDocument, fallbackTarget: ScrollTarget): ScrollTarget {
  // 滚动容器已下沉到 .mod-library__content（仅卡片区域滚动），返回顶部应滚该容器而非 .app-surface。
  const modLibraryContent = documentLike.querySelector(".mod-library__content");
  if (modLibraryContent && typeof modLibraryContent.scrollTo === "function") {
    return modLibraryContent;
  }
  return fallbackTarget;
}

export function scrollModLibraryBackToTop(target: ScrollTarget) {
  target.scrollTo({ top: 0, behavior: "smooth" });
}
