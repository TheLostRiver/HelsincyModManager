import assert from "node:assert/strict";
import { test } from "node:test";
import { projectExternalCardBadge } from "./externalCardBadge.ts";
import { externalStateCopy } from "./externalStateCopy.ts";

const zh = externalStateCopy.zh_cn.badge;

function stateOf(summaryState, counts, extra = {}) {
  return {
    summary: {
      state: summaryState,
      matchedFileCount: counts.matched ?? 0,
      missingFileCount: counts.missing ?? 0,
      changedFileCount: counts.changed ?? 0,
      unreadableFileCount: counts.unreadable ?? 0,
      occupiedBy: [],
      files: [],
    },
    stale: false,
    lastError: null,
    ...extra,
  };
}

const mixedState = stateOf("mixed", { changed: 2, unreadable: 1, missing: 2 });

// ---- 9c 全占用改口的 fixture ----
// 与真机场景同形：one001 双胞胎字节相同，HMM 装了 flat，扫 wrapped——两个文件
// 都「一致」且都被 flat 的清单条目认领。（名字 ↔ 沙箱 id 以 results.json 的
// display_name 为准：flat = mod-import-1787939077192-1。）
const flat = { modId: "mod-import-1787939077192-1", modName: "weapon-mod-one001-flat" };
const other = { modId: "mod-other", modName: "另一把太刀" };

function fileFact(targetPath, state, claimant) {
  return claimant
    ? {
        targetPath,
        state,
        claimedByModId: claimant.modId,
        ...(claimant.modName !== undefined ? { claimedByModName: claimant.modName } : {}),
      }
    : { targetPath, state };
}

/** 全部文件都被 `occupiers` 轮流认领的 summary（默认哈希态 installed / 全一致）。 */
function fullyOccupiedState(occupiers, options = {}) {
  const state = options.state ?? "installed";
  const fileState = options.fileState ?? "matched";
  const files = [
    fileFact("nativePC/wp/one/one001/mod/one001.mod3", fileState, occupiers[0]),
    fileFact("nativePC/wp/one/one001/mod/one001.mrl3", fileState, occupiers[1] ?? occupiers[0]),
  ];
  const counts =
    fileState === "matched" ? { matched: files.length } : { [fileState]: files.length };
  return stateOf(state, counts, {
    stale: options.stale ?? false,
    summary: {
      ...stateOf(state, counts).summary,
      occupiedBy: occupiers,
      files,
    },
  });
}

function projectFor(externalState, viewMode) {
  return projectExternalCardBadge({
    installStatus: "not_installed",
    externalState,
    viewMode,
    copy: zh,
  });
}

test("门禁：只有 manifest 不认领的卡片才显示徽标，残留结果不得上卡片", () => {
  // 已安装/异常态即使 Map 里残留安装前的扫描结果，也必须维持既有状态显示。
  for (const installStatus of ["installed", "disabled", "conflict", "unknown", "repair_required"]) {
    assert.equal(
      projectExternalCardBadge({
        installStatus,
        externalState: mixedState,
        viewMode: "classic",
        copy: zh,
      }),
      null,
      `${installStatus} 不得渲染外部徽标`,
    );
  }
});

test("门禁：本会话没扫过（无记录或无 summary）时不渲染徽标", () => {
  for (const externalState of [null, undefined, stateOf("mixed", {}, { summary: null })]) {
    assert.equal(
      projectExternalCardBadge({
        installStatus: "not_installed",
        externalState,
        viewMode: "classic",
        copy: zh,
      }),
      null,
    );
  }
});

test("档位随视图路由：tech 完整、classic/grid 精简、list 极简", () => {
  const project = (viewMode) =>
    projectExternalCardBadge({
      installStatus: "not_installed",
      externalState: mixedState,
      viewMode,
      copy: zh,
    });

  assert.equal(project("tech").text, "已被改动 · 3 个文件 · 另有 2 个缺失");
  assert.equal(project("classic").text, "已改动 3 · 缺失 2");
  assert.equal(project("grid").text, "已改动 3 · 缺失 2");
  // 极简档只报总数，不假装知道分类。
  assert.equal(project("list").text, "需注意 5");
});

test("label 恒用完整档事实并带外部前缀，与视图档位无关", () => {
  const badge = projectExternalCardBadge({
    installStatus: "not_installed",
    externalState: stateOf("partial", { missing: 3 }),
    viewMode: "list",
    copy: zh,
  });

  assert.equal(badge.text, "需注意 3");
  assert.equal(badge.label, "外部 · 部分安装 · 3 个文件缺失");
  assert.equal(badge.case, "partial");
  assert.equal(badge.stale, false);
});

test("stale 结果在 label 追加过时提示并透传标志", () => {
  const badge = projectExternalCardBadge({
    installStatus: "not_installed",
    externalState: stateOf("changed", { changed: 1 }, { stale: true }),
    viewMode: "grid",
    copy: zh,
  });

  assert.equal(badge.stale, true);
  assert.ok(badge.label.endsWith(` · ${zh.staleHint}`), "label 末尾必须是过时提示");

  const fresh = projectExternalCardBadge({
    installStatus: "not_installed",
    externalState: stateOf("changed", { changed: 1 }),
    viewMode: "grid",
    copy: zh,
  });
  assert.ok(!fresh.label.includes(zh.staleHint), "非 stale 不得出现过时提示");
});

test("case 透传：installed / not_installed / unknown 也产出徽标", () => {
  for (const [state, expectedText] of [
    ["installed", zh.installed],
    ["not_installed", zh.notInstalled],
    ["unknown", zh.unknown],
  ]) {
    const badge = projectExternalCardBadge({
      installStatus: "not_installed",
      externalState: stateOf(state, {}),
      viewMode: "classic",
      copy: zh,
    });
    assert.equal(badge.case, state);
    assert.equal(badge.text, expectedText);
  }
});

test("三语 staleHint 均非空（供 title/aria 合成）", () => {
  for (const locale of ["zh_cn", "en", "ja"]) {
    assert.ok(
      externalStateCopy[locale].badge.staleHint.length > 0,
      `${locale} 缺 staleHint`,
    );
  }
});

// ---- 9c：比对集全部被其他 HMM MOD 占用时卡片改口 ----

test("全占用（单占用者）：三档都改口，带名字的档位用显示名，case 为 occupied", () => {
  const state = fullyOccupiedState([flat]);

  // 完整档是自然句；精简档事实前置、名字在尾（截断先丢名字）；极简档只说事实。
  assert.equal(projectFor(state, "tech").text, "已被「weapon-mod-one001-flat」占用");
  assert.equal(projectFor(state, "classic").text, "已被占用 · weapon-mod-one001-flat");
  assert.equal(projectFor(state, "grid").text, "已被占用 · weapon-mod-one001-flat");
  assert.equal(projectFor(state, "list").text, "已被占用");
  for (const viewMode of ["tech", "classic", "grid", "list"]) {
    assert.equal(projectFor(state, viewMode).case, "occupied", `${viewMode} 的 case`);
  }
});

test("全占用的 label 用完整档且不带「外部」前缀——占用者是 HMM 自己的 MOD", () => {
  const badge = projectFor(fullyOccupiedState([flat]), "list");

  assert.equal(badge.label, "已被「weapon-mod-one001-flat」占用");
  assert.ok(!badge.label.includes(zh.externalOrigin), "「外部」正是要消灭的误导词");
  assert.equal(badge.stale, false);

  // stale 的过时提示照常追加，与哈希徽标同一规则。
  const stale = projectFor(fullyOccupiedState([flat], { stale: true }), "grid");
  assert.equal(stale.stale, true);
  assert.equal(stale.label, `已被「weapon-mod-one001-flat」占用 · ${zh.staleHint}`);
});

test("全占用（多占用者）：报数量不报名字，数量取去重后的占用者数", () => {
  const state = fullyOccupiedState([flat, other]);

  assert.equal(projectFor(state, "tech").text, "已被 2 个 MOD 占用");
  assert.equal(projectFor(state, "grid").text, "已被占用 · 2 个 MOD");
  assert.equal(projectFor(state, "list").text, "已被占用");
  assert.equal(projectFor(state, "list").label, "已被 2 个 MOD 占用");
});

test("占用者显示名缺席（MOD 已删）时回退 id，绝不空白", () => {
  const gone = { modId: "mod-gone" };
  const badge = projectFor(fullyOccupiedState([gone]), "tech");

  assert.equal(badge.text, "已被「mod-gone」占用");
});

test("全占用改口与哈希判定正交：文件全是「已被改动」也照样改口", () => {
  // 另一个 MOD 装了不同内容到同一路径：哈希说 changed，清单说归它——后者才是可定位的事实。
  const state = fullyOccupiedState([other], { state: "changed", fileState: "changed" });

  const badge = projectFor(state, "classic");
  assert.equal(badge.case, "occupied");
  assert.equal(badge.text, "已被占用 · 另一把太刀");
});

test("部分占用维持哈希徽标：哪怕只有一个文件无主，也不改口", () => {
  const files = [
    fileFact("nativePC/wp/one/one001/mod/one001.mod3", "matched", flat),
    fileFact("nativePC/wp/one/one001/mod/one001.mrl3", "matched", null),
  ];
  const state = stateOf("installed", { matched: 2 }, {
    summary: { ...stateOf("installed", { matched: 2 }).summary, occupiedBy: [flat], files },
  });

  const badge = projectFor(state, "classic");
  assert.equal(badge.case, "installed");
  assert.equal(badge.text, zh.installed);
  assert.equal(badge.label, `${zh.externalOrigin} · ${zh.installed}`);
});

test("空比对集（unknown）不算全占用：空集上的 every 恒真，但什么都没证明", () => {
  const badge = projectFor(stateOf("unknown", {}), "classic");

  assert.equal(badge.case, "unknown");
  assert.equal(badge.text, zh.unknown);
});

test("防御：每个文件都带占用标记但 occupiedBy 为空的矛盾 DTO，退回哈希徽标", () => {
  const files = [
    fileFact("nativePC/wp/one/one001/mod/one001.mod3", "matched", flat),
    fileFact("nativePC/wp/one/one001/mod/one001.mrl3", "matched", flat),
  ];
  const state = stateOf("installed", { matched: 2 }, {
    summary: { ...stateOf("installed", { matched: 2 }).summary, occupiedBy: [], files },
  });

  const badge = projectFor(state, "tech");
  assert.equal(badge.case, "installed");
  assert.ok(!badge.text.includes("0"), "不得凭空说「已被 0 个 MOD 占用」");
});

test("门禁在改口之前：已安装态的卡片即使残留全占用结果也不渲染", () => {
  assert.equal(
    projectExternalCardBadge({
      installStatus: "installed",
      externalState: fullyOccupiedState([flat]),
      viewMode: "classic",
      copy: zh,
    }),
    null,
  );
});
