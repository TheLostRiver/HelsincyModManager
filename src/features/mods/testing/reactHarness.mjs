import assert from "node:assert/strict";
import { readFileSync, existsSync } from "node:fs";
import { createRequire, registerHooks } from "node:module";
import { fileURLToPath, pathToFileURL, URL } from "node:url";
import { setTimeout, clearTimeout } from "node:timers";
import path from "node:path";
import ts from "typescript";
import React, { act } from "react";
import TestRenderer from "react-test-renderer";

const featureUrl = new URL("../", import.meta.url);
const require = createRequire(import.meta.url);
const reactUrls = new Map(["react", "react/jsx-runtime", "react/jsx-dev-runtime"].map(
  (name) => [name, pathToFileURL(require.resolve(name)).href],
));
const stubs = {
  event: `export function listen(name, callback) {
    const api = globalThis.__hmmReactTest;
    if (api.listenerFails) return Promise.reject(new Error("fixture listener failure"));
    api.progress.add(callback);
    return Promise.resolve(() => api.progress.delete(callback));
  }`,
  webview: `export function getCurrentWebview() { return { onDragDropEvent(callback) {
    const api = globalThis.__hmmReactTest;
    api.drop.add(callback);
    return Promise.resolve(() => api.drop.delete(callback));
  } }; }`,
  feedback: `export function useFeedback() { return globalThis.__hmmReactTest.feedback; }`,
  i18n: `export function useI18n() { return { locale: globalThis.__hmmReactTest.locale }; }
    export function resolveCopy(copy, locale) { return copy[locale] ?? copy.en; }`,
  storage: `export function useModStorageSettings() { return { writesFrozen: globalThis.__hmmReactTest.frozen }; }`,
  storageTypes: `export function getModStorageFreezeReason(value) { return value ? "storage-frozen" : undefined; }`,
  importApi: `export function previewDroppedModArchives(paths) { return globalThis.__hmmReactTest.preview(paths); }
    export function startImportModTask(input) { return globalThis.__hmmReactTest.startImport(input); }`,
  overlay: `export function ModImportDropOverlay(props) { globalThis.__hmmReactTest.overlay = props; return null; }`,
};
stubs.cacheEvent = stubs.event.replace("api.listenerFails", "api.listenerFails || api.cacheListenerFails");
const substitutions = new Map([
  ["@tauri-apps/api/event", "event"], ["@tauri-apps/api/webview", "webview"],
  ["../../shared/feedback", "feedback"], ["../../shared/i18n", "i18n"],
  ["../settings/ModStorageSettingsProvider", "storage"], ["../settings/modStorageTypes", "storageTypes"],
  ["./modImportApi", "importApi"], ["./ModImportDropOverlay", "overlay"],
]);

// Load the actual TS/TSX modules; replace only IPC and presentation boundaries.
registerHooks({
  resolve(specifier, context, nextResolve) {
    if (reactUrls.has(specifier)) return { url: reactUrls.get(specifier), shortCircuit: true };
    if (specifier === "@tauri-apps/api/event" && context.parentURL?.endsWith("/ModLibrarySessionCacheProvider.tsx")) {
      return { url: "hmm-test:cacheEvent", shortCircuit: true };
    }
    if (context.parentURL?.startsWith(featureUrl.href) && substitutions.has(specifier)) {
      return { url: `hmm-test:${substitutions.get(specifier)}`, shortCircuit: true };
    }
    if (specifier.startsWith(".") && context.parentURL?.startsWith(featureUrl.href)) {
      const file = fileURLToPath(new URL(specifier, context.parentURL));
      for (const candidate of [file, `${file}.ts`, `${file}.tsx`, `${file}.js`]) {
        if (path.extname(candidate) && existsSync(candidate)) {
          return { url: pathToFileURL(candidate).href, shortCircuit: true };
        }
      }
    }
    return nextResolve(specifier, context);
  },
  load(url, context, nextLoad) {
    if (url.startsWith("hmm-test:")) return { format: "module", source: stubs[url.slice(9)], shortCircuit: true };
    if (url.startsWith(featureUrl.href) && /\.tsx?$/.test(url)) {
      return {
        format: "module", shortCircuit: true,
        source: ts.transpileModule(readFileSync(fileURLToPath(url), "utf8"), {
          fileName: fileURLToPath(url),
          compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022, jsx: ts.JsxEmit.ReactJSX },
        }).outputText,
      };
    }
    return nextLoad(url, context);
  },
});

export const loadFeature = (name) => import(new URL(name, featureUrl).href);
const { ModImportDropProvider } = await loadFeature("ModImportDropProvider.tsx");
const { ModLibrarySessionCacheProvider, useModLibrarySessionCache } = await loadFeature("ModLibrarySessionCacheProvider.tsx");
const { useModLibraryQuery } = await loadFeature("useModLibraryQuery.ts");
export { act };
export const options = { timeout: 5000, concurrency: false };
export const profileKey = "profile:mhw\u0000test-profile";
const profile = { gameId: "mhw", profileId: "test-profile" };
const all = { kind: "all" };

globalThis.IS_REACT_ACT_ENVIRONMENT = true;
globalThis.window = { localStorage: { getItem: () => null, setItem() {} }, setTimeout, clearTimeout };

export const pageOf = (name, page = 1) => ({
  items: [{ id: name, name, status: "not_installed", categoryLabels: [] }],
  page, pageSize: 24, libraryTotal: 48, matchingTotal: 48,
});
function deferred() {
  let resolve, reject;
  const promise = new Promise((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
}

function runtime(listenerFails = false) {
  const api = { locale: "en", frozen: null, listenerFails, progress: new Set(), drop: new Set(), starts: [], notices: new Map(), toasts: [] };
  api.preview = async (paths) => paths.map((archivePath) => ({ archivePath, fileName: archivePath, sizeBytes: 10, errorCode: null, warningCode: null }));
  api.feedback = {
    pushToast: (toast) => api.toasts.push(toast),
    showTaskNotice: (notice) => api.notices.set(notice.taskId, notice),
    dismissTaskNotice: (taskId) => api.notices.delete(taskId),
  };
  api.emit = (taskId, status = "completed", error = null, kind = "mod_import") => {
    for (const callback of api.progress) callback({ payload: {
      taskId, kind, status, phase: status === "failed" ? "mod_import.unpack.failed"
        : status === "queued" ? "mod_import.queued" : "mod_import.prepare.completed",
      current: null, total: null, message: null, error, resultRef: null,
    } });
  };
  api.startImport = async ({ archivePath }) => {
    const taskId = `fixture-${api.starts.length + 1}`;
    api.starts.push({ archivePath, taskId });
    api.emit(taskId, "queued");
    return { kind: "mod_import", status: "queued", taskId };
  };
  api.drag = (paths) => { for (const callback of api.drop) callback({ payload: { type: "drop", paths } }); };
  globalThis.__hmmReactTest = api;
  return api;
}

export async function mountDrop(t, { cacheListenerFails = false } = {}) {
  const api = runtime();
  api.cacheListenerFails = cacheListenerFails;
  function Capture({ children }) { api.cache = useModLibrarySessionCache(); return children; }
  const tree = (route) => React.createElement(React.StrictMode, null,
    React.createElement(ModLibrarySessionCacheProvider, null,
      React.createElement(Capture, null,
        React.createElement(ModImportDropProvider, null, React.createElement("route", { key: route })))));
  let root;
  await act(async () => { root = TestRenderer.create(tree("mods")); });
  t.after(async () => { await act(async () => root.unmount()); });
  assert.equal(api.drop.size, 1);
  assert.equal(api.progress.size, cacheListenerFails ? 1 : 2, "One cache observer and one queue watcher survive StrictMode");
  assert.equal(api.overlay.listenerReady, true);
  return { api, changeRoute: async (route) => { await act(async () => root.update(tree(route))); } };
}

export async function mountQuery(t, { listenerFails = false } = {}) {
  const api = runtime(listenerFails);
  const state = {};
  const pending = [];
  const loadPage = (input) => { const request = { input, ...deferred() }; pending.push(request); return request.promise; };
  function QueryProbe({ profileContext = profile }) {
    state.query = useModLibraryQuery({ rawSearch: "", filter: all, profileContext, loadPage, cache: state.cache });
    return null;
  }
  function Host({ show, profileContext }) {
    state.cache = useModLibrarySessionCache();
    return show ? React.createElement(QueryProbe, { profileContext }) : null;
  }
  const tree = (props) => React.createElement(ModLibrarySessionCacheProvider, null, React.createElement(Host, props));
  let root;
  await act(async () => { root = TestRenderer.create(tree({ show: false })); });
  await act(async () => root.update(tree({ show: true })));
  t.after(async () => { await act(async () => root.unmount()); });
  return {
    api, state, pending,
    update: async (props) => { await act(async () => root.update(tree({ show: true, ...props }))); },
    resolve: async (request, page) => { await act(async () => request.resolve(page)); },
  };
}

export function loadAfterWriteCallback(refresh, cache) {
  const url = new URL("ModLibraryPage.tsx", featureUrl);
  const source = readFileSync(url, "utf8");
  const ast = ts.createSourceFile(fileURLToPath(url), source, ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
  const declarations = [];
  function visit(node) {
    if (ts.isVariableDeclaration(node) && node.name.getText(ast) === "refreshModLibraryAfterWrite") declarations.push(node);
    ts.forEachChild(node, visit);
  }
  visit(ast);
  assert.equal(declarations.length, 1, "Exactly one production write callback is required");
  return new Function("useCallback", "resetContentScroll", "refreshModLibrary", "librarySessionCache",
    `return (${declarations[0].initializer.getText(ast)});`)((fn) => fn, () => {}, refresh, cache);
}
