import assert from "node:assert/strict";
import { test } from "node:test";

import {
  WEAPON_REPLACEMENT_ERROR_CODES,
  isWeaponReplacementErrorCode,
  replacementErrorCode,
  replacementErrorMessage,
} from "./replacementErrorText.ts";

const FALLBACK = "替换目标信息读取失败";

test("每个武器稳定码都有具体文案，不落回兜底提示", () => {
  for (const code of WEAPON_REPLACEMENT_ERROR_CODES) {
    const message = replacementErrorMessage({ code }, FALLBACK);
    assert.notEqual(message, FALLBACK, `${code} 缺少具体文案`);
    assert.ok(message.length > 0);
  }
});

test("武器码附带可复制的诊断码，通用流程错误不附带", () => {
  const weapon = replacementErrorMessage({ code: "weapon_binary_format_invalid" }, FALLBACK);
  assert.ok(weapon.includes("（诊断码：weapon_binary_format_invalid）"));

  const generic = replacementErrorMessage({ code: "replacement_target_already_selected" }, FALLBACK);
  assert.ok(!generic.includes("诊断码"));
  assert.equal(generic, "当前目标已安装。");
});

test("文案不回显路径、offset 或二进制内部字段", () => {
  for (const code of WEAPON_REPLACEMENT_ERROR_CODES) {
    const message = replacementErrorMessage({ code }, FALLBACK);
    assert.ok(!message.includes("nativePC/wp/"), `${code} 回显了资源路径`);
    assert.ok(!/0x[0-9a-f]+/i.test(message), `${code} 回显了偏移或魔数`);
  }
});

test("未知码与非错误对象回落到兜底文案", () => {
  assert.equal(replacementErrorMessage({ code: "weapon_future_code" }, FALLBACK), FALLBACK);
  assert.equal(replacementErrorMessage(new Error("boom"), FALLBACK), FALLBACK);
  assert.equal(replacementErrorMessage(null, FALLBACK), FALLBACK);
  assert.equal(replacementErrorMessage("weapon_unknown_part", FALLBACK), FALLBACK);
});

test("码提取与武器码判定", () => {
  assert.equal(replacementErrorCode({ code: "task_not_found" }), "task_not_found");
  assert.equal(replacementErrorCode({ code: 42 }), null);
  assert.equal(replacementErrorCode(undefined), null);
  assert.ok(isWeaponReplacementErrorCode("weapon_cross_family_target"));
  assert.ok(!isWeaponReplacementErrorCode("plan_token_invalid"));
});

test("跨武器类型的目标给出可执行建议", () => {
  const message = replacementErrorMessage({ code: "weapon_cross_family_target" }, FALLBACK);
  assert.ok(message.includes("请选择同一类武器作为目标。"));
});
