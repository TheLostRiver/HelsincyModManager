import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "node:test";
import {
  externalStatusAriaLabel,
  fileClaimantDisplayName,
  occupierDisplayName,
  projectExternalStatusBadge,
} from "./externalInstallStatusView.ts";
import {
  externalStateCopy,
  externalStateErrorMessage,
} from "./externalStateCopy.ts";

const zh = externalStateCopy.zh_cn;

function summaryOf(state, counts, files = []) {
  return {
    state,
    matchedFileCount: counts.matched ?? 0,
    missingFileCount: counts.missing ?? 0,
    changedFileCount: counts.changed ?? 0,
    unreadableFileCount: counts.unreadable ?? 0,
    files,
  };
}

test("完整档按决策关键度排序：改动在前，缺失在后", () => {
  const badge = projectExternalStatusBadge(
    summaryOf("mixed", { changed: 1, unreadable: 1, missing: 3 }),
    "tech",
    zh.badge,
  );

  assert.equal(badge.tier, "full");
  assert.equal(badge.text, "已被改动 · 2 个文件 · 另有 3 个缺失");
});

test("精简档与极简档按视图降级，极简档不假装知道分类", () => {
  const summary = summaryOf("mixed", { changed: 1, unreadable: 1, missing: 3 });

  const compact = projectExternalStatusBadge(summary, "grid", zh.badge);
  assert.equal(compact.tier, "compact");
  assert.equal(compact.text, "已改动 2 · 缺失 3");

  const minimal = projectExternalStatusBadge(summary, "list", zh.badge);
  assert.equal(minimal.tier, "minimal");
  // 极简档只报总数，不声称「N 个缺失」——96px 里放不下分类，宁可少说不说错。
  assert.equal(minimal.text, "需注意 5");
});

test("mixed 无缺失时不渲染「另有 0 个缺失」尾段", () => {
  const badge = projectExternalStatusBadge(
    summaryOf("mixed", { changed: 0, unreadable: 2, missing: 0 }),
    "tech",
    zh.badge,
  );

  assert.equal(badge.text, "已被改动 · 2 个文件");
});

test("aria 标签带外部来源前缀且用完整档事实", () => {
  const badge = projectExternalStatusBadge(
    summaryOf("partial", { missing: 3 }),
    "list",
    zh.badge,
  );

  assert.equal(badge.text, "需注意 3");
  assert.equal(
    externalStatusAriaLabel(badge, zh.badge),
    "外部 · 部分安装 · 3 个文件缺失",
  );
});

test("错误码映射：已知码取词，未知码保留原码可见", () => {
  assert.equal(externalStateErrorMessage("external_state_scan_stale", zh), zh.errors.stale);
  assert.equal(
    externalStateErrorMessage("external_state_scan_cancelled", zh),
    zh.errors.cancelled,
  );
  const unknown = externalStateErrorMessage("external_state_scan_totally_new", zh);
  assert.match(unknown, /external_state_scan_totally_new/);
});

test("三语字典的徽标函数在同一输入下都产出非空文案", () => {
  const numbers = { changed: 1, unreadable: 0, missing: 2 };
  for (const locale of ["zh_cn", "en", "ja"]) {
    const copy = externalStateCopy[locale];
    for (const badgeCase of ["partial", "changed", "mixed"]) {
      for (const tier of ["full", "compact", "minimal"]) {
        const text = copy.badge[badgeCase][tier](numbers);
        assert.ok(text.length > 0, `${locale}.${badgeCase}.${tier} 不得为空`);
      }
    }
  }
});

// ---- 占用归因（#286 第三层）----

test("占用者展示名：有名用名，MOD 已删回退 id，绝不空白", () => {
  assert.equal(
    occupierDisplayName({ modId: "mod-flat", modName: "Flat 武器" }),
    "Flat 武器",
  );
  assert.equal(occupierDisplayName({ modId: "mod-gone" }), "mod-gone");
});

test("文件行占用标签：无占用为 null，有占用按名字回退 id", () => {
  const base = { targetPath: "nativePC/a.mod3", state: "matched" };
  assert.equal(fileClaimantDisplayName(base), null);
  assert.equal(
    fileClaimantDisplayName({ ...base, claimedByModId: "mod-flat", claimedByModName: "Flat 武器" }),
    "Flat 武器",
  );
  assert.equal(
    fileClaimantDisplayName({ ...base, claimedByModId: "mod-gone" }),
    "mod-gone",
  );
});

test("三语占用文案：提示行织入全部占用者名与数量，行内标签织入名字", () => {
  const expectations = {
    zh_cn: { joined: "甲、乙", tagProbe: "PROBE_NAME" },
    en: { joined: "甲, 乙", tagProbe: "PROBE_NAME" },
    ja: { joined: "甲、乙", tagProbe: "PROBE_NAME" },
  };
  for (const locale of ["zh_cn", "en", "ja"]) {
    const copy = externalStateCopy[locale];
    const notice = copy.occupiedNotice(["甲", "乙"], 3);
    assert.ok(
      notice.includes(expectations[locale].joined),
      `${locale} 提示行必须按本语言分隔符连接占用者名：${notice}`,
    );
    assert.ok(notice.includes("3"), `${locale} 提示行必须织入占用文件数`);
    assert.match(
      copy.fileClaimedBy("PROBE_NAME"),
      /PROBE_NAME/,
      `${locale} 行内标签必须包含占用者名`,
    );
  }
});

test("三语全占用徽标（9c）：单占用者的完整/精简档织入名字、多占用者织入数量，极简档非空且不带名字", () => {
  for (const locale of ["zh_cn", "en", "ja"]) {
    const occupied = externalStateCopy[locale].badge.occupied;
    for (const tier of ["full", "compact"]) {
      assert.match(
        occupied[tier](["PROBE_NAME"]),
        /PROBE_NAME/,
        `${locale}.occupied.${tier} 单占用者必须带名字`,
      );
      const several = occupied[tier](["甲", "乙", "丙"]);
      assert.ok(several.includes("3"), `${locale}.occupied.${tier} 多占用者必须报数量：${several}`);
      assert.ok(
        !several.includes("甲"),
        `${locale}.occupied.${tier} 多占用者不列名字（列不下，且细节在弹窗）：${several}`,
      );
    }
    const minimal = occupied.minimal(["PROBE_NAME"]);
    assert.ok(minimal.length > 0, `${locale}.occupied.minimal 不得为空`);
    assert.ok(!minimal.includes("PROBE_NAME"), `${locale}.occupied.minimal 96px 里放不下名字`);
  }
});

test("Section 形状：提示行按 occupiedBy 门禁，行内标签经回退助手取名", () => {
  const currentDirectory = dirname(fileURLToPath(import.meta.url));
  const sectionSource = readFileSync(
    join(currentDirectory, "ExternalStateSection.tsx"),
    "utf8",
  );

  // 提示行只在存在占用者时渲染，数量取「有占用标记的文件数」而非占用者数。
  assert.match(sectionSource, /summary\.occupiedBy\.length > 0 \?/);
  assert.match(sectionSource, /summary\.occupiedBy\.map\(occupierDisplayName\)/);
  assert.match(
    sectionSource,
    /summary\.files\.filter\(\(file\) => file\.claimedByModId !== undefined\)\.length/,
  );
  // 行内标签：经 fileClaimantDisplayName（名字 ?? id）取名，null 不渲染。
  assert.match(sectionSource, /fileClaimantDisplayName\(file\)/);
  assert.match(sectionSource, /copy\.fileClaimedBy\(claimant\)/);
  assert.match(sectionSource, /mod-detail-dialog__external-claimed/);
  assert.match(sectionSource, /is-occupied/);
});
