import assert from "node:assert/strict";
import { test } from "node:test";

import {
  isExternalImportDisplayText,
  isExternalImportOpaqueId,
  isExternalImportSourceDto,
  isPlainRecord,
} from "./externalImportTypes.ts";

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

test("external import validators provide one shared fail-closed boundary", () => {
  const controlCharacter = String.fromCharCode(0);

  assert.equal(isPlainRecord({ candidateId: "candidate-a" }), true);
  assert.equal(isPlainRecord([]), false);
  assert.equal(isExternalImportOpaqueId("candidate-a"), true);
  assert.equal(isExternalImportOpaqueId("candidate/path"), false);
  assert.equal(isExternalImportDisplayText("人工候选"), true);
  assert.equal(isExternalImportDisplayText(`unsafe${controlCharacter}label`), false);
});
