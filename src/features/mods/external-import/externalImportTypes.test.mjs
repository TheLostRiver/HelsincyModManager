import assert from "node:assert/strict";
import { test } from "node:test";

import { isExternalImportSourceDto } from "./externalImportTypes.ts";

function source(overrides = {}) {
  return {
    sourceId: "source-a",
    adapterId: "hunting_box_directory_v1",
    displayLabel: "Hunting Box directory",
    expiresAtUnixMillis: 1_000,
    ...overrides,
  };
}

test("external import source DTO rejects path-like, unsafe, and malformed values", () => {
  const controlCharacter = String.fromCharCode(0);

  assert.equal(isExternalImportSourceDto(source()), true);
  assert.equal(isExternalImportSourceDto(source({ displayLabel: "C:\\synthetic\\external-source" })), false);
  assert.equal(isExternalImportSourceDto(source({ displayLabel: `unsafe${controlCharacter}label` })), false);
  assert.equal(isExternalImportSourceDto(source({ displayLabel: "x".repeat(161) })), false);
  assert.equal(isExternalImportSourceDto(source({ sourceId: "source/path" })), false);
  assert.equal(isExternalImportSourceDto(source({ expiresAtUnixMillis: 1.5 })), false);
});
