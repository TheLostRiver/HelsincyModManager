import { existsSync, readFileSync } from "node:fs";
import { createRequire, registerHooks } from "node:module";
import { fileURLToPath, pathToFileURL, URL } from "node:url";
import ts from "typescript";

const sourceRoot = new URL("../../", import.meta.url);
const require = createRequire(import.meta.url);
const reactUrls = new Map(["react", "react/jsx-runtime", "react/jsx-dev-runtime"].map(
  (name) => [name, pathToFileURL(require.resolve(name)).href],
));

// Execute production TS/TSX without copying hook logic; callers name only external/leaf stubs.
export function registerReactTestModules(stubs = {}, packages = {}) {
  const modules = new Map(Object.entries(stubs).map(([path, source]) => [new URL(path, sourceRoot).href, source]));
  const packageUrls = new Map(Object.entries(packages).map(([name, source]) => {
    const url = `hmm-test:${encodeURIComponent(name)}`;
    modules.set(url, source);
    return [name, url];
  }));
  return registerHooks({
    resolve(specifier, context, nextResolve) {
      if (packageUrls.has(specifier)) return { url: packageUrls.get(specifier), shortCircuit: true };
      if (reactUrls.has(specifier)) return { url: reactUrls.get(specifier), shortCircuit: true };
      if (specifier.startsWith(".") && context.parentURL?.startsWith(sourceRoot.href)) {
        const base = new URL(specifier, context.parentURL).href;
        for (const url of [base, `${base}.ts`, `${base}.tsx`, `${base}.js`, `${base}/index.ts`]) {
          if (/\.[cm]?[jt]sx?$/.test(url) && (modules.has(url) || existsSync(fileURLToPath(url)))) {
            return { url, shortCircuit: true };
          }
        }
      }
      return nextResolve(specifier, context);
    },
    load(url, context, nextLoad) {
      if (modules.has(url)) return { format: "module", source: modules.get(url), shortCircuit: true };
      if (url.startsWith(sourceRoot.href) && url.endsWith(".css")) return { format: "module", source: "", shortCircuit: true };
      if (url.startsWith(sourceRoot.href) && /\.tsx?$/.test(url)) {
        return {
          format: "module",
          shortCircuit: true,
          source: ts.transpileModule(readFileSync(fileURLToPath(url), "utf8"), {
            fileName: fileURLToPath(url),
            compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022, jsx: ts.JsxEmit.ReactJSX },
          }).outputText,
        };
      }
      return nextLoad(url, context);
    },
  });
}
