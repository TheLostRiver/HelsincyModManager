import js from "@eslint/js";
import globals from "globals";
import reactHooks from "eslint-plugin-react-hooks";
import tseslint from "typescript-eslint";

export default tseslint.config(
  js.configs.recommended,
  ...tseslint.configs.recommended,
  {
    files: ["scripts/**/*.mjs", "src/**/*.test.mjs"],
    languageOptions: {
      ecmaVersion: 2022,
      globals: {
        ...globals.node,
        ...globals.es2022,
      },
    },
  },
  {
    files: ["**/*.{ts,tsx}"],
    languageOptions: {
      ecmaVersion: 2022,
      globals: {
        ...globals.browser,
        ...globals.es2022,
      },
    },
    plugins: {
      "react-hooks": reactHooks,
    },
    rules: {
      ...reactHooks.configs.recommended.rules,
    },
  },
  {
    // armor-data 是本地候选数据与抓取工具；tmp、.planning、.plan-attestation 是临时
    // 诊断脚本、计划与证明产物。四者都在 .gitignore 里，不纳入版本管理，也不该被
    // lint——里面的脚本会用到 console / process / setTimeout 这类只在本机跑才有的全局。
    ignores: [
      ".claude",
      ".plan-attestation",
      ".planning",
      ".vite",
      ".worktrees",
      "armor-data",
      "dist",
      "src-tauri/target",
      "target",
      "tmp",
    ],
  },
);
