import assert from "node:assert/strict";
import { test } from "node:test";
import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { registerReactTestModules } from "../../shared/testing/reactModuleLoader.mjs";

registerReactTestModules();
const { I18nContext } = await import("../../shared/i18n/I18nProvider.tsx");
const { modLibraryCopy } = await import("./modLibraryCopy.ts");
const { ModLibraryPagination } = await import("./ModLibraryPagination.tsx");
const { ModLibraryPageControls } = await import("./ModLibraryPageControls.tsx");
const copy = modLibraryCopy.zh_cn.pagination;

function render(Component, props) {
  return renderToStaticMarkup(React.createElement(I18nContext.Provider, {
    value: { locale: "zh_cn", preference: "zh_cn", systemLocale: "zh_cn", setPreference() {} },
  }, React.createElement(Component, props)));
}

test("empty results do not reserve a footer", () => {
  for (const result of [null, { page: 1, pageSize: 24, matchingTotal: 0 }]) {
    assert.equal(render(ModLibraryPagination, {
      pageSize: 24, result, onPageSizeChange() {}, onPageChange() {},
    }), "");
  }
});

test("single-page results keep capacity and result information inside the footer without page buttons", () => {
  for (const matchingTotal of [1, 16, 24]) {
    const result = { page: 1, pageSize: 24, matchingTotal };
    const controls = render(ModLibraryPagination, {
      pageSize: 24, result, onPageSizeChange() {}, onPageChange() {},
    });
    assert.match(controls, /^<footer[\s\S]*<\/footer>$/);
    assert.doesNotMatch(controls, /<nav|aria-current="page"/);
    assert.ok(controls.includes(copy.perPageSizeAria(24)));
    assert.ok(controls.includes(copy.items(matchingTotal)));
    assert.match(controls, /aria-live="polite"/);
    assert.match(controls, /aria-atomic="true"/);
    assert.ok(controls.includes(copy.range(1, matchingTotal, matchingTotal)));
  }
});

test("multi-page results expose the actual current page and all labeled navigation actions", () => {
  const html = render(ModLibraryPagination, {
    pageSize: 24, result: { page: 2, pageSize: 24, matchingTotal: 49 },
    onPageSizeChange() {}, onPageChange() {},
  });
  for (const label of [copy.gotoFirst, copy.gotoPrev, copy.gotoNext, copy.gotoLast,
    copy.pageAria(1), copy.pageAria(2), copy.pageAria(3)]) {
    assert.ok(html.includes('aria-label="' + label + '"'));
  }
  assert.equal((html.match(/aria-current="page"/g) ?? []).length, 1);
  assert.ok(html.includes('aria-label="' + copy.pageAria(2) + '" aria-current="page"'));
  assert.match(html, /aria-haspopup="listbox"/, "capacity and navigation share the footer");
});

test("refreshing a multi-page result keeps navigation visible but unavailable", () => {
  const html = render(ModLibraryPagination, {
    pageSize: 24, result: { page: 1, pageSize: 24, matchingTotal: 49 },
    busy: true, onPageSizeChange() {}, onPageChange() {},
  });
  const buttons = (html.match(/<button/g) ?? []).length;
  assert.ok(buttons > 0);
  assert.equal((html.match(/aria-disabled="true"/g) ?? []).length, buttons);
});

test("pending capacity changes keep range and page buttons tied to the displayed query snapshot", () => {
  const html = render(ModLibraryPagination, {
    pageSize: 192,
    result: { page: 2, pageSize: 24, matchingTotal: 100 },
    busy: true,
    onPageSizeChange() {},
    onPageChange() {},
  });
  assert.ok(html.includes(copy.perPageSizeAria(192)));
  assert.ok(html.includes(copy.busyRange(copy.range(25, 48, 100))));
  assert.ok(html.includes(copy.compactRange(25, 48, 100)));
  assert.doesNotMatch(html, /role="listbox"/);
  assert.match(html, /aria-disabled="true"/);
  assert.ok(html.includes('aria-label="' + copy.pageAria(5) + '"'));
});

test("a larger committed capacity hides page buttons but keeps the control available to reduce it", () => {
  const html = render(ModLibraryPagination, {
    pageSize: 192, result: { page: 1, pageSize: 192, matchingTotal: 100 },
    onPageSizeChange() {}, onPageChange() {},
  });
  assert.ok(html.includes(copy.perPageSizeAria(192)));
  assert.doesNotMatch(html, /<nav|aria-disabled="true"/);
});

test("initial loading does not announce an empty library before results arrive", () => {
  const html = render(ModLibraryPageControls, {
    pageSize: 24, result: null, busy: true, onPageSizeChange() {},
  });
  assert.ok(html.includes(copy.busyLabel));
  assert.ok(!html.includes(copy.emptyRange));
});
