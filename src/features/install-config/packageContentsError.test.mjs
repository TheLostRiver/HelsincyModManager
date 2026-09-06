import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

import {
  classifyPackageContentsError,
  packageContentsErrorCode,
} from "./packageContentsError.ts";

test("从后端错误对象里取出稳定错误码", () => {
  assert.equal(
    packageContentsErrorCode({ code: "imported_mod_file_scan_unavailable", message: "..." }),
    "imported_mod_file_scan_unavailable",
  );
});

test("拿不到码的各种形态一律返回 null，不抛", () => {
  for (const value of [null, undefined, "boom", 42, {}, { code: 7 }, { message: "x" }, []]) {
    assert.equal(packageContentsErrorCode(value), null, `${JSON.stringify(value)} 不该产出码`);
  }
});

test("陈旧内容根单独成档——它是唯一有恢复路径的失败", () => {
  assert.equal(
    classifyPackageContentsError({ code: "imported_mod_file_scan_stale_content_root_choice" }),
    "stale-content-root",
  );
});

test("其余失败归为通用档", () => {
  for (const code of [
    "imported_mod_file_scan_unavailable",
    "imported_mod_file_scan_unsupported_entry",
    "imported_mod_file_scan_depth_limit_exceeded",
    "package_contents_sandbox_unavailable",
    "package_contents_mod_not_found",
    "package_contents_content_root_not_a_candidate",
  ]) {
    assert.equal(classifyPackageContentsError({ code }), "generic", `${code} 应归通用档`);
  }
  assert.equal(classifyPackageContentsError(new Error("network")), "generic");
});

/*
 * 码是跨语言契约：Rust 改了字面量而前端没跟，恢复路径会静默消失——玩家看到的
 * 依然是「读不到包内容」，只是再也没有那个「清除选择」的出路。仓库里没有覆盖
 * 这一族的契约门禁（`tauriContractCoverage` 只管命令名，`replacementErrorCodeContract`
 * 只管 replacement 那一族），所以在这里自己钉住。
 */
test("陈旧内容根的码与 Rust 侧字面量一致", () => {
  const source = readFileSync("src-tauri/crates/hmm-ports/src/mod_import.rs", "utf8");

  assert.match(
    source,
    /Self::StaleContentRootChoice\s*=>\s*"imported_mod_file_scan_stale_content_root_choice"/,
    "Rust 侧的码改了，前端的恢复路径判据要同步",
  );
});
