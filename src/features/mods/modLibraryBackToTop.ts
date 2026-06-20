type ScrollTarget = {
  scrollTo: (options: ScrollToOptions) => void;
};

type QueryDocument = {
  querySelector: (selector: string) => ScrollTarget | null;
};

export function getModLibraryBackToTopTarget(documentLike: QueryDocument, fallbackTarget: ScrollTarget): ScrollTarget {
  const appSurface = documentLike.querySelector(".app-surface");
  if (appSurface && typeof appSurface.scrollTo === "function") {
    return appSurface;
  }
  return fallbackTarget;
}

export function scrollModLibraryBackToTop(target: ScrollTarget) {
  target.scrollTo({ top: 0, behavior: "smooth" });
}
