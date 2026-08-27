import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

function readSource(path) {
  return readFileSync(path, "utf8");
}

test("history panel stays a pure read-only query surface", () => {
  const panel = readSource(
    "src/features/mods/external-import/ExternalImportHistoryPanel.tsx",
  );
  const hook = readSource(
    "src/features/mods/external-import/useExternalImportHistory.ts",
  );
  const styles = readSource(
    "src/features/mods/external-import/ExternalImportAction.css",
  );

  // 结构与可访问性:列表语义、加载/错误播报。
  assert.match(panel, /role="list"/);
  assert.match(panel, /role="listitem"/);
  assert.match(panel, /role="status" aria-live="polite"/);
  assert.match(panel, /role="alert"/);
  assert.match(panel, /aria-expanded=\{expanded\}/);

  // 保留期与「不可从历史重试」必须明示,记录凭空消失比没有记录更伤信任。
  assert.match(panel, /history\.retentionHint/);
  assert.match(panel, /history\.retryHint/);

  // drill-down 行主体复用当次结果的显示名兜底口径。
  assert.match(panel, /\{result\.displayName \?\? extCopy\.preview\.unnamed\}/);

  // 历史是纯查询:不持有任务事件 listener,不提供从历史重试,不触碰文件系统面。
  assert.doesNotMatch(`${panel}\n${hook}`, /listen<TaskProgressEventDto>|listen\(/);
  assert.doesNotMatch(`${panel}\n${hook}`, /retryExternalImportBatch|startExternalImportBatch/);
  assert.doesNotMatch(
    `${panel}\n${hook}`,
    /readFile|writeFile|removeFile|convertFileSrc|asset:|thumbnail:|sandbox|cache|archivePath/i,
  );

  // hook 只消费两个只读 command,drill-down 用放宽版守卫(允许非终态批次)。
  assert.match(hook, /listExternalImportBatches/);
  assert.match(hook, /getExternalImportBatchResult/);
  assert.match(hook, /isExternalImportHistoryBatchResultPage/);
  assert.match(hook, /isExternalImportHistoryPage/);

  // 渲染必须保持纯:locale/文案只在提交后同步进 ref。渲染期写 ref 会让被 React 丢弃的
  // 渲染泄漏未提交的语言,异步请求回来时用上 UI 从未展示过的文案。
  assert.match(
    hook,
    /useEffect\(\(\) => \{\s*extCopyRef\.current = extCopy;\s*localeRef\.current = locale;\s*\}, \[extCopy, locale\]\);/,
  );
  // 两处赋值各只允许出现一次,且只能是上面 effect 里那一次。
  assert.equal(hook.match(/extCopyRef\.current = /g)?.length, 1);
  assert.equal(hook.match(/localeRef\.current = /g)?.length, 1);

  // 记录多时在弹窗内部滚动,不撑高整个 Dialog。
  assert.match(styles, /\.external-import__history-list \{[\s\S]{0,240}max-height/);
});
