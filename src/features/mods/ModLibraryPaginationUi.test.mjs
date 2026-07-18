import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

function readPaginationSource(fileName) {
  return readFileSync(new URL(`./${fileName}`, import.meta.url), "utf8");
}

test("pagination UI uses the shared helpers and a custom upward page-size listbox", () => {
  const source = readPaginationSource("ModLibraryPagination.tsx");

  assert.match(source, /getModLibraryItemRange/);
  assert.match(source, /getModLibraryPageSlots/);
  assert.match(source, /getModLibraryTotalPages/);
  assert.match(source, /role="listbox"/);
  assert.match(source, /role="option"/);
  assert.match(source, /aria-haspopup="listbox"/);
  assert.match(source, /tabIndex=\{focusedPageSizeIndex === optionIndex \? 0 : -1\}/);
  assert.doesNotMatch(source, /<select|<option/);
});

test("page-size keyboard navigation moves focus separately from committing a value", () => {
  const source = readPaginationSource("ModLibraryPagination.tsx");
  const keyboardHandlerStart = source.indexOf("const handlePageSizeOptionKeyDown");
  const keyboardHandlerEnd = source.indexOf("const requestPage", keyboardHandlerStart);
  const keyboardHandler = source.slice(keyboardHandlerStart, keyboardHandlerEnd);
  const arrowDownStart = keyboardHandler.indexOf('case "ArrowDown"');
  const arrowUpStart = keyboardHandler.indexOf('case "ArrowUp"');
  const enterStart = keyboardHandler.indexOf('case "Enter"');
  const arrowBranches = keyboardHandler.slice(arrowDownStart, enterStart);
  const commitBranches = keyboardHandler.slice(enterStart);

  assert.ok(arrowDownStart >= 0);
  assert.ok(arrowUpStart > arrowDownStart);
  assert.match(arrowBranches, /focusPageSizeOption/);
  assert.doesNotMatch(arrowBranches, /commitPageSize|onPageSizeChange/);
  assert.match(commitBranches, /case " ":/);
  assert.match(commitBranches, /commitPageSize/);
  assert.match(source, /requestAnimationFrame\(\(\) => pageSizeTriggerRef\.current\?\.focus\(\)\)/);
  assert.match(
    source,
    /const handlePointerDown[\s\S]*?pageSizeRootRef\.current\?\.contains[\s\S]*?closePageSizeMenu\(false\)/,
  );
  assert.match(source, /const handleEscape[\s\S]*?event\.key === "Escape"[\s\S]*?closePageSizeMenu\(true\)/);
});

test("pagination UI exposes labeled Lucide navigation and a complete live range", () => {
  const source = readPaginationSource("ModLibraryPagination.tsx");

  for (const icon of ["ChevronsLeft", "ChevronLeft", "ChevronRight", "ChevronsRight"]) {
    assert.match(source, new RegExp(`<${icon}\\s`));
  }
  for (const label of ["前往第一页", "前往上一页", "前往下一页", "前往最后一页"]) {
    assert.match(source, new RegExp(`aria-label="${label}"`));
  }
  for (const tooltip of ["第一页", "上一页", "下一页", "最后一页"]) {
    assert.match(source, new RegExp(`content="${tooltip}" describeControl=\\{false\\}`));
  }
  assert.match(source, /ModLibraryControlTooltip/);
  assert.doesNotMatch(source, /title="(?:第一页|上一页|下一页|最后一页)"/);
  assert.match(source, /aria-current=\{slot === currentPage \? "page" : undefined\}/);
  assert.match(source, /aria-live="polite"/);
  assert.match(source, /aria-atomic="true"/);
  assert.match(source, /aria-disabled=\{busy \|\| undefined\}/);
});

test("busy pagination closes the menu without dropping focus from its controls", () => {
  const source = readPaginationSource("ModLibraryPagination.tsx");
  const css = readPaginationSource("ModLibraryPagination.css");

  assert.match(source, /if \(busy && pageSizeMenuOpen\) \{\s*closePageSizeMenu\(true\);/);
  assert.match(source, /const openPageSizeMenu = useCallback\(\(\) => \{\s*if \(busy\)/);
  assert.match(source, /const commitPageSize = \(nextPageSize:[\s\S]*?if \(busy\)/);
  assert.match(source, /aria-expanded=\{pageSizeMenuOpen && !busy\}/);
  assert.match(source, /aria-disabled=\{busy \|\| undefined\}/);
  assert.doesNotMatch(source, /disabled=\{busy\}/);
  assert.match(source, /\{pageSizeMenuOpen && !busy \? \(/);
  assert.match(css, /\.mod-library-pagination__page-size-trigger\[aria-disabled="true"\]/);
  assert.match(css, /\.mod-library-pagination__page-size-trigger:hover:not\(\[aria-disabled="true"\]\)/);
  assert.doesNotMatch(css, /\.mod-library-pagination__[^{]+:disabled/);
});

test("pagination footer stays a compact semantic-token toolbar without overlay styling", () => {
  const css = readPaginationSource("ModLibraryPagination.css");
  const rootRule = css.match(/\.mod-library-pagination\s*\{([\s\S]*?)\n\}/)?.[1] ?? "";
  const layoutRule = css.match(/\.mod-library-pagination__layout\s*\{([\s\S]*?)\n\}/)?.[1] ?? "";
  const listboxRule = css.match(
    /\.mod-library-pagination__page-size-listbox\s*\{([\s\S]*?)\n\}/,
  )?.[1] ?? "";

  assert.match(layoutRule, /block-size:\s*48px/);
  assert.doesNotMatch(rootRule, /position:\s*(?:fixed|sticky|absolute)/);
  assert.match(listboxRule, /bottom:\s*calc\(100% \+ 8px\)/);
  assert.match(listboxRule, /border-radius:\s*8px/);
  assert.doesNotMatch(css, /box-shadow/);
  assert.doesNotMatch(css, /#[0-9a-f]{3,8}\b/i);
  assert.match(css, /inline-size:\s*32px/);
  assert.match(css, /block-size:\s*32px/);
  assert.match(css, /@media\s*\(prefers-reduced-motion:\s*reduce\)/);
});
