import assert from "node:assert/strict";
import { test } from "node:test";

import { replacementCopy } from "./replacementCopy.ts";
import {
  WEAPON_REPLACEMENT_ERROR_CODES,
  isWeaponReplacementErrorCode,
  replacementErrorCode,
  replacementErrorMessage,
} from "./replacementErrorText.ts";

const FALLBACK = "替换目标信息读取失败";
const LOCALES = ["zh_cn", "en", "ja"];
const zhErrors = replacementCopy.zh_cn.errors;

test("每个武器稳定码在三种语言下都有具体文案，不落回兜底提示", () => {
  for (const locale of LOCALES) {
    for (const code of WEAPON_REPLACEMENT_ERROR_CODES) {
      const message = replacementErrorMessage({ code }, FALLBACK, replacementCopy[locale].errors);
      assert.notEqual(message, FALLBACK, `${locale}/${code} 缺少具体文案`);
      assert.ok(message.length > 0);
    }
  }
});

test("武器码附带可复制的诊断码，通用流程错误不附带", () => {
  const weapon = replacementErrorMessage({ code: "weapon_binary_format_invalid" }, FALLBACK, zhErrors);
  assert.ok(weapon.includes("（诊断码：weapon_binary_format_invalid）"));

  const generic = replacementErrorMessage(
    { code: "replacement_target_already_selected" },
    FALLBACK,
    zhErrors,
  );
  assert.ok(!generic.includes("诊断码"));
  assert.equal(generic, "当前目标已安装。");
});

/**
 * 脱敏要求见 docs/WEAPON_RETARGET_DESIGN.md：不回显路径、offset、material 名或二进制内容。
 *
 * `nativePC/wp` 这类固定结构约定是允许的——它对每个 Mod 都一样，不携带任何用户数据；
 * 被禁的是能定位到具体文件或具体武器的东西，所以下面按形态而不是按字面量断言。
 */
const FORBIDDEN_DETAIL_PATTERNS = [
  { pattern: /[A-Za-z]:[\\/]/, label: "Windows 绝对路径" },
  // 分隔符必须同时认 / 和 \：Windows 侧的相对资源路径用反斜杠，且不带盘符，
  // 只认 / 的话 nativePC\wp\... 会整条漏过去。
  { pattern: /[\\/][A-Za-z0-9_.-]+[\\/][A-Za-z0-9_.-]+/, label: "多级资源路径" },
  { pattern: /\bwp[\\/][A-Za-z0-9_.-]+/i, label: "具体武器目录" },
  { pattern: /0x[0-9a-f]+/i, label: "偏移或魔数" },
  { pattern: /\b[0-9a-f]{16,}\b/i, label: "哈希或二进制摘要" },
  { pattern: /\b(?:offset|sha256|material|AppData|Users)\b/i, label: "内部字段或系统路径片段" },
];

test("文案不回显路径、offset 或二进制内部字段", () => {
  for (const code of WEAPON_REPLACEMENT_ERROR_CODES) {
    const message = replacementErrorMessage({ code }, FALLBACK, zhErrors);
    for (const { pattern, label } of FORBIDDEN_DETAIL_PATTERNS) {
      assert.ok(!pattern.test(message), `${code} 回显了${label}：${message}`);
    }
  }
});

test("脱敏断言本身有效——真回显了细节就会红", () => {
  // 防止上面的模式表退化成永远为真的空断言。
  const leaks = [
    "读取 D:\\Games\\MHW\\nativePC\\wp\\w01 失败。",
    "nativePC/wp/w01/w01.mod3 解析失败。",
    // 无盘符的相对反斜杠路径：上面那条 D:\ 样本是被盘符规则抓住的，
    // 掩盖了"多级路径规则只认正斜杠"这个洞。
    "nativePC\\wp\\one\\one001\\mod\\one001.mrl3 解析失败。",
    "material 校验在 offset 0x1f40 处失败。",
    "文件 sha256 为 9f2c4b1ae7d05631 的资源不匹配。",
  ];

  for (const leak of leaks) {
    assert.ok(
      FORBIDDEN_DETAIL_PATTERNS.some(({ pattern }) => pattern.test(leak)),
      `脱敏模式表没能识别泄漏样本：${leak}`,
    );
  }
});

test("未知码与非错误对象回落到兜底文案", () => {
  assert.equal(replacementErrorMessage({ code: "weapon_future_code" }, FALLBACK, zhErrors), FALLBACK);
  assert.equal(replacementErrorMessage(new Error("boom"), FALLBACK, zhErrors), FALLBACK);
  assert.equal(replacementErrorMessage(null, FALLBACK, zhErrors), FALLBACK);
  assert.equal(replacementErrorMessage("weapon_unknown_part", FALLBACK, zhErrors), FALLBACK);
});

test("码提取与武器码判定", () => {
  assert.equal(replacementErrorCode({ code: "task_not_found" }), "task_not_found");
  assert.equal(replacementErrorCode({ code: 42 }), null);
  assert.equal(replacementErrorCode(undefined), null);
  assert.ok(isWeaponReplacementErrorCode("weapon_cross_family_target"));
  assert.ok(!isWeaponReplacementErrorCode("plan_token_invalid"));
});

test("跨武器类型的目标给出可执行建议", () => {
  const message = replacementErrorMessage({ code: "weapon_cross_family_target" }, FALLBACK, zhErrors);
  assert.ok(message.includes("请选择同一类武器作为目标。"));
});
