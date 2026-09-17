import assert from "node:assert/strict";
import { test } from "node:test";
import { act, loadFeature, mountQuery, pageOf, options } from "./testing/reactHarness.mjs";

const { deleteModFromLibrary } = await loadFeature("modDeleteApi.ts");
const { loadModLibraryPageWithStatuses } = await loadFeature("modLibraryPageLoader.ts");

function deferred() {
  let resolve, reject;
  const promise = new Promise((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
}

async function fixture(t, ids) {
  const catalog = new Set(ids);
  let catalogReads = 0, revision = 0;
  const h = await mountQuery(t, { loadPage: (input, context, cache) => loadModLibraryPageWithStatuses(input, context, {
    cache,
    query: async () => {
      catalogReads++;
      return { ...pageOf("unused"), items: [...catalog].map((id) => pageOf(id).items[0]),
        libraryTotal: catalog.size, matchingTotal: catalog.size };
    },
    states: async ({ gameId, profileId, modIds }) => ({ gameId, profileId, modIds, epoch: "delete", revision: ++revision,
      reset: false, available: true, summaries: modIds.map((modId) => ({ profileId, modId, status: "not_installed",
        managedFileCount: 0, backupCount: 0 })) }),
    scan: async () => assert.fail("catalog deletion does not require an integrity scan"),
  }) });
  return { ...h, catalog, catalogReads: () => catalogReads };
}

for (const leavePage of [false, true]) {
  test(`deletion refreshes catalog membership and counts${leavePage ? " after leaving the page" : " on the current page"}`, options, async (t) => {
    const h = await fixture(t, ["removed", "kept"]);
    const done = deferred();
    h.api.invoke = async (command, args) => {
      assert.equal(command, "delete_mod_from_library");
      assert.deepEqual(args, { modId: "removed" });
      await done.promise;
      h.catalog.delete(args.modId);
      return { modId: args.modId, removedRevisionCount: 1, removedPackageIds: [] };
    };
    let deletion;
    await act(async () => { deletion = deleteModFromLibrary("removed"); });
    assert.equal(h.catalogReads(), 1);
    assert.deepEqual(h.state.query.page.items.map((item) => item.id), ["removed", "kept"]);
    if (leavePage) await h.update({ show: false });
    await act(async () => { done.resolve(); await deletion; });
    if (leavePage) await h.update({ show: true });
    await act(async () => h.state.query.synchronize());
    assert.deepEqual(h.state.query.page.items.map((item) => item.id), ["kept"]);
    assert.equal(h.state.query.page.libraryTotal, 1);
    assert.equal(h.state.query.page.matchingTotal, 1);
    assert.equal(h.state.query.statusTrusted, true);
    assert.equal(h.catalogReads(), 2);
  });
}

test("batch deletion reconciles once after success and a failure following a catalog change", options, async (t) => {
  const h = await fixture(t, ["first", "second", "kept"]);
  let token;
  await act(async () => { token = h.state.cache.beginWrite(); });
  h.api.invoke = async (command, { modId }) => {
    assert.equal(command, "delete_mod_from_library");
    assert.equal(h.state.cache.isWriting(), true);
    h.catalog.delete(modId);
    if (modId === "second") throw new Error("reply failed after catalog update");
    return { modId, removedRevisionCount: 1, removedPackageIds: [] };
  };
  await act(async () => { await deleteModFromLibrary("first"); });
  await act(async () => { await assert.rejects(deleteModFromLibrary("second"), /reply failed/); });
  assert.equal(h.catalogReads(), 1, "the outer batch boundary prevents intermediate queries");
  assert.equal(h.state.query.writing, true);
  await act(async () => { h.state.cache.finishWrite(token); });
  await act(async () => h.state.query.synchronize());
  assert.deepEqual(h.state.query.page.items.map((item) => item.id), ["kept"]);
  assert.equal(h.state.query.page.libraryTotal, 1);
  assert.equal(h.state.query.statusTrusted, true);
  assert.equal(h.catalogReads(), 2);
});
