// #286 3b-2「A+」：会话表的纯逻辑——作用域换新与读取隔离。
// 每条用例都跑过控制组：把实现退回去，确认它会变红。

import assert from "node:assert/strict";
import { test } from "node:test";
import {
  EMPTY_EXTERNAL_STATE_SESSION,
  externalStateResultsForScope,
  recordExternalStateResult,
  sameExternalStateScope,
} from "./externalStateSession.ts";

const scopeA = { gameId: "mhw", profileId: "default" };
const scopeB = { gameId: "mhw", profileId: "second" };
const dto = (state) => ({
  summary: { state, matchedFileCount: 0, missingFileCount: 0, changedFileCount: 0, unreadableFileCount: 0, occupiedBy: [], files: [] },
  stale: false,
  lastError: null,
});

test("同一作用域内逐条累积，且不改动传入的旧表", () => {
  const first = recordExternalStateResult(EMPTY_EXTERNAL_STATE_SESSION, scopeA, "mod-a", dto("installed"));
  const second = recordExternalStateResult(first, scopeA, "mod-b", dto("partial"));

  assert.deepEqual([...second.results.keys()], ["mod-a", "mod-b"]);
  assert.equal(second.results.get("mod-a").summary.state, "installed");
  // 不可变：旧快照不受后续记录影响（React 依赖引用变化重渲染）。
  assert.deepEqual([...first.results.keys()], ["mod-a"]);
  assert.equal(EMPTY_EXTERNAL_STATE_SESSION.results.size, 0);
  assert.equal(EMPTY_EXTERNAL_STATE_SESSION.scope, null);
});

test("记录到不同作用域时整表换新——别的配置档的结果不能留下来", () => {
  const inA = recordExternalStateResult(EMPTY_EXTERNAL_STATE_SESSION, scopeA, "mod-a", dto("installed"));
  const inB = recordExternalStateResult(inA, scopeB, "mod-b", dto("changed"));

  assert.deepEqual(inB.scope, scopeB);
  assert.deepEqual([...inB.results.keys()], ["mod-b"], "旧作用域的 mod-a 必须被换掉");
});

test("读取：作用域缺失或不匹配一律空表，匹配才返回结果", () => {
  const inA = recordExternalStateResult(EMPTY_EXTERNAL_STATE_SESSION, scopeA, "mod-a", dto("installed"));

  assert.equal(externalStateResultsForScope(inA, scopeA).size, 1);
  assert.equal(externalStateResultsForScope(inA, { ...scopeA }).size, 1, "按字段比较，不看引用");
  assert.equal(externalStateResultsForScope(inA, scopeB).size, 0, "别的配置档读到的必须是空表");
  assert.equal(externalStateResultsForScope(inA, { gameId: "other", profileId: "default" }).size, 0, "gameId 也参与作用域");
  assert.equal(externalStateResultsForScope(inA, null).size, 0, "配置档未就绪时空表");
  assert.equal(externalStateResultsForScope(EMPTY_EXTERNAL_STATE_SESSION, scopeA).size, 0);
});

test("作用域比较看 gameId 与 profileId 两项", () => {
  assert.equal(sameExternalStateScope(scopeA, { ...scopeA }), true);
  assert.equal(sameExternalStateScope(scopeA, scopeB), false);
  assert.equal(sameExternalStateScope(scopeA, { gameId: "other", profileId: "default" }), false);
});
