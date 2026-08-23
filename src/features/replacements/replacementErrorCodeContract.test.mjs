import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

import { replacementCopy } from "./replacementCopy.ts";
import {
  WEAPON_REPLACEMENT_ERROR_CODES,
  replacementErrorMessage,
} from "./replacementErrorText.ts";

/**
 * 前后端替换错误码的集合级契约。
 *
 * 后端把稳定码原样透传给前端（src-tauri/src/replacement_commands.rs 的
 * analysis_error_to_command_error 对 AnalysisRejected { code } 直接返回 code），
 * 前端按码映射文案。缺文案的码会退回无信息量的兜底提示——这正是 WR-04 记录的缺陷。
 *
 * Rust 侧 weapon_error_code_contract.rs 的穷尽 match 只能挡住"忘了在 Rust 里补码"；
 * 它挡不住"补了 Rust 却没补前端文案"——那种情况 cargo test、tsc、eslint 全绿而用户受损。
 * 真正的跨语言闸门在这里：任一侧新增、删除或改名，本文件先红。
 *
 * 前端测试读 Rust 源码的先例见 profileSaveDirectoryDiscovery.test.mjs。
 */

const FALLBACK = "替换目标信息读取失败";

const REPLACEMENT_COMMANDS = "src-tauri/src/replacement_commands.rs";

const RUST_WEAPON_ERROR_ENUMS = [
  {
    file: "src-tauri/crates/hmm-games-mhw/src/weapon_retarget/analysis.rs",
    enumName: "WeaponAnalysisError",
  },
  {
    file: "src-tauri/crates/hmm-games-mhw/src/weapon_retarget/binary.rs",
    enumName: "WeaponBinaryError",
  },
];

/**
 * 取指定枚举 `impl` 块里 `code()` 的函数体。
 * 必须限定在 impl 块内：同文件还有 WeaponAnalysisWarning::code()，
 * 它的 weapon_partial_part_set 是 warning 码，不属于错误文案表。
 */
function codeFunctionBody(source, enumName) {
  const implStart = source.indexOf(`impl ${enumName} {`);
  assert.notEqual(implStart, -1, `未找到 impl ${enumName}`);

  // impl 块以列 0 的 } 收尾；match 与 fn 的收尾大括号都有缩进，不会误命中。
  const implEnd = source.indexOf("\n}\n", implStart);
  assert.notEqual(implEnd, -1, `impl ${enumName} 没有正常结束`);

  const implBlock = source.slice(implStart, implEnd);
  const signature = /pub fn code\(self\)\s*->\s*&'static str\s*\{/.exec(implBlock);
  assert.notEqual(signature, null, `${enumName}::code 的签名变了，契约解析已失效`);

  return implBlock.slice(signature.index);
}

function weaponCodesFromRust({ file, enumName }) {
  const body = codeFunctionBody(readFileSync(file, "utf8"), enumName);

  assert.ok(
    !/(?:^|[\s(|])_\s*=>/.test(body),
    `${enumName}::code 出现通配 arm，会静默吞掉新变体`,
  );

  const codes = [
    ...body.matchAll(/Self::\w+(?:\s*\|\s*Self::\w+)*\s*=>\s*"([a-z0-9_]+)"/g),
  ].map((match) => match[1]);

  // arm 数必须与解析出的字面码数一致。否则说明有 arm 不是 `Self::X => "码"` 形态
  // （例如改成返回常量或函数调用），解析会漏掉它、契约变成假绿。
  const armCount = (body.match(/=>/g) ?? []).length;
  assert.equal(
    codes.length,
    armCount,
    `${enumName}::code 有 ${armCount} 个 arm，却只解析出 ${codes.length} 个字面码`,
  );

  return codes;
}

function allRustWeaponCodes() {
  return RUST_WEAPON_ERROR_ENUMS.flatMap(weaponCodesFromRust);
}

test("前端武器文案表与 Rust code() 的集合完全一致", () => {
  const rustCodes = allRustWeaponCodes();
  assert.ok(rustCodes.length > 0, "没有从 Rust 源码解析出任何武器码");

  assert.deepEqual(
    [...rustCodes].sort(),
    [...WEAPON_REPLACEMENT_ERROR_CODES].sort(),
    "两侧码集合必须一致：后端新增或改名后没同步 replacementErrorText.ts，用户会看到兜底提示",
  );
});

test("Rust 侧武器码全局唯一，前端才能按码做穷尽映射", () => {
  const rustCodes = allRustWeaponCodes();
  assert.equal(new Set(rustCodes).size, rustCodes.length, "武器稳定码跨枚举重复");
});

test("replacement_commands.rs 吐出的每个码在三种语言下都有前端文案", () => {
  // 通用码是散落的字面量，没有单一枚举可穷尽，所以按命名约定抓。
  // 抓到非错误码的字符串时本测试会红，那时补 allowlist 即可——比静默漏码好。
  // generic 表不像 weapon 表有 tsc 穷尽闸门，必须逐语言扫：漏写 en/ja 时这里先红。
  const source = readFileSync(REPLACEMENT_COMMANDS, "utf8");
  const codes = [
    ...new Set(
      [...source.matchAll(/"((?:replacement|weapon|reinstall|plan|task)_[a-z0-9_]+)"/g)].map(
        (match) => match[1],
      ),
    ),
  ];

  assert.ok(codes.length > 0, `没有从 ${REPLACEMENT_COMMANDS} 解析出任何错误码`);

  for (const locale of ["zh_cn", "en", "ja"]) {
    const uncovered = codes.filter(
      (code) =>
        replacementErrorMessage({ code }, FALLBACK, replacementCopy[locale].errors) === FALLBACK,
    );
    assert.deepEqual(
      uncovered,
      [],
      `这些后端码在 ${locale} 会退回兜底提示，需要在 replacementCopy.ts 补文案`,
    );
  }
});
