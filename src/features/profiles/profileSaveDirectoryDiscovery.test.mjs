import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { test } from "node:test";

function readSource(path) {
  return readFileSync(path, "utf8");
}

const forbiddenDiscoveryFields = new RegExp(
  [
    "raw" + "Path",
    "full" + "Path",
    "steam" + "Id64",
    "account" + "Id",
    "x" + "ml",
    "profile" + "Url",
  ].join("|"),
  "i",
);

test("save directory discovery API uses opaque candidate ids without raw filesystem or steam ids", () => {
  assert.equal(existsSync("src/features/profiles/profileSaveDirectoryDiscoveryApi.ts"), true);
  assert.equal(existsSync("src/features/profiles/profileSaveDirectoryDiscoveryTypes.ts"), true);

  const api = readSource("src/features/profiles/profileSaveDirectoryDiscoveryApi.ts");
  const types = readSource("src/features/profiles/profileSaveDirectoryDiscoveryTypes.ts");

  assert.match(api, /invoke<SaveDirectoryDiscoveryDto>\("discover_profile_save_directories",\s*input\)/);
  assert.match(api, /invoke<SaveDirectoryDiscoveryDto>\("confirm_profile_save_directory_candidate"/);
  assert.match(api, /discoveryId:\s*input\.discoveryId/);
  assert.match(api, /candidateId:\s*input\.candidateId/);
  assert.doesNotMatch(api, forbiddenDiscoveryFields);

  assert.match(types, /candidateId:\s*string/);
  assert.match(types, /discoveryId:\s*string/);
  assert.match(types, /accountName:\s*string\s*\|\s*null/);
  assert.match(types, /avatarUrl:\s*string\s*\|\s*null/);
  assert.match(types, /pathLabel:\s*string/);
  assert.match(types, /savedSettings\?:\s*ProfileDirectorySelectionDto\s*\|\s*null/);
  assert.doesNotMatch(types, forbiddenDiscoveryFields);
});
