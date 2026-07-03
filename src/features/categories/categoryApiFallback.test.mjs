import assert from "node:assert/strict";
import { test } from "node:test";

import {
  createCategory,
  deleteCategory,
  listCategories,
  updateCategory,
} from "./categoryApi.ts";

test("category API provides an interactive local store outside Tauri", async () => {
  const initial = await listCategories();
  assert.ok(initial.length >= 5);
  assert.deepEqual(
    initial.map((category) => category.sortOrder),
    initial.map((_, index) => index),
  );

  const createdId = await createCategory({
    name: "测试分类",
    color: "#0EA5E9",
    sortOrder: 99,
  });

  let categories = await listCategories();
  assert.equal(categories.at(-1)?.id, createdId);
  assert.equal(categories.at(-1)?.name, "测试分类");
  assert.equal(categories.at(-1)?.color, "#0EA5E9");

  await updateCategory({
    categoryId: createdId,
    name: "测试分类改名",
    color: null,
    sortOrder: 1,
  });

  categories = await listCategories();
  assert.equal(categories[1].id, createdId);
  assert.equal(categories[1].name, "测试分类改名");
  assert.equal(categories[1].color, undefined);

  await deleteCategory(createdId);
  categories = await listCategories();
  assert.equal(categories.some((category) => category.id === createdId), false);

  for (const category of categories) {
    await deleteCategory(category.id);
  }
  categories = await listCategories();
  assert.deepEqual(categories, []);
});
