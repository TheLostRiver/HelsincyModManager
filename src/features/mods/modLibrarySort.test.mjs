import assert from "node:assert/strict";
import { test } from "node:test";
import { DEFAULT_MOD_LIBRARY_SORT, MOD_LIBRARY_SORTS, MOD_LIBRARY_SORT_STORAGE_KEY, formatModContentSize, modLibraryNameSortKey, normalizeModLibraryText, readModLibrarySort, sortBrowserModLibrary, writeModLibrarySort } from "./modLibrarySort.ts";

const fixtures = [
  ["a", "Mod 10", 20, 300], ["b", "Mod 2", 10, undefined], ["c", "Mod 1", undefined, 0],
  ["d", "Mod 02", 10, 100], ["e", "mod 2", 30, 100], ["f", "Alpha", undefined, undefined], ["g", "装备 3", 20, 100],
].map(([id, name, importedAtUnixMillis, contentSizeBytes]) => ({ id, name, importedAtUnixMillis, contentSizeBytes }));
const expected = {
  name_asc: ["f", "c", "b", "d", "e", "a", "g"], name_desc: ["g", "a", "e", "d", "b", "c", "f"],
  imported_at_asc: ["b", "d", "a", "g", "e", "f", "c"], imported_at_desc: ["e", "a", "g", "b", "d", "f", "c"],
  size_asc: ["c", "d", "e", "g", "a", "f", "b"], size_desc: ["a", "d", "e", "g", "c", "f", "b"],
};

test("six rules handle numeric names, zero sizes, unknowns and stable ties without mutating input", () => {
  const original = structuredClone(fixtures);
  for (const sort of MOD_LIBRARY_SORTS) {
    assert.deepEqual(sortBrowserModLibrary(fixtures, sort).map(({ id }) => id), expected[sort]);
  }
  assert.deepEqual(fixtures, original);
});

test("natural name keys share the backend byte contract and Unicode normalization", () => {
  assert.deepEqual([...modLibraryNameSortKey("Mod 02")], [109, 111, 100, 32, 48, 0, 0, 0, 0, 0, 0, 0, 1, 50, 0]);
  assert.deepEqual(modLibraryNameSortKey("ＭＯＤ　2"), modLibraryNameSortKey("Mod 02"));
  assert.equal(normalizeModLibraryText("  Café\u0085İ　"), "café i\u0307");
  assert.deepEqual(modLibraryNameSortKey("Cafe\u0301"), modLibraryNameSortKey("Café"));
  assert.deepEqual(sortBrowserModLibrary([{ id: "a", name: "Mod 99999999999999999999999" }, { id: "b", name: "Mod 100000000000000000000000" }], "name_asc").map(({ id }) => id), ["a", "b"]);
});

test("sort preference survives a storage round trip and invalid values restore newest import", () => {
  const values = new Map();
  const storage = { getItem: (key) => values.get(key) ?? null, setItem: (key, value) => values.set(key, value) };
  assert.equal(readModLibrarySort(storage), DEFAULT_MOD_LIBRARY_SORT);
  for (const rule of MOD_LIBRARY_SORTS) {
    writeModLibrarySort(storage, rule);
    assert.equal(readModLibrarySort(storage), rule);
  }
  values.set(MOD_LIBRARY_SORT_STORAGE_KEY, "old-or-corrupt-sort");
  assert.equal(readModLibrarySort(storage), DEFAULT_MOD_LIBRARY_SORT);
  const broken = { getItem() { throw new Error("unavailable"); }, setItem() { throw new Error("quota"); } };
  assert.equal(readModLibrarySort(broken), DEFAULT_MOD_LIBRARY_SORT);
  assert.equal(writeModLibrarySort(broken, "size_desc"), "size_desc");
});

test("size labels use numeric metadata and keep missing or invalid values unknown", () => {
  assert.equal(formatModContentSize(0, "en"), "0 B");
  assert.equal(formatModContentSize(1536, "en"), "1.5 KiB");
  assert.equal(formatModContentSize(1048576, "en"), "1 MiB");
  for (const unknown of [undefined, null, NaN, Infinity, -1, "3 MB", Number.MAX_SAFE_INTEGER + 1]) {
    assert.equal(formatModContentSize(unknown, "en"), null);
  }
});
