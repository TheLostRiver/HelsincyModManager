import assert from "node:assert/strict";
import test from "node:test";
import {
  buildProbeCases,
  evaluateRenderState,
  summarizeNetworkEvents,
} from "./check-thumbnail-protocol.mjs";

test("probe covers every url shape the handler must accept", () => {
  const cases = buildProbeCases({
    packageId: "pkg-1",
    variant: "preview-768",
    contentHash: "abc123",
  });

  assert.equal(cases.length, 4);
  assert.equal(cases[0].url, "thumbnail://pkg-1/preview-768/abc123");
  assert.equal(cases[1].url, "http://thumbnail.localhost/pkg-1/preview-768/abc123");
  assert.equal(cases[2].url, "http://thumbnail.pkg-1/preview-768/abc123");
  assert.match(cases[3].url, /does-not-exist/);
});

test("probe includes a control case that must fail", () => {
  const cases = buildProbeCases({
    packageId: "pkg-1",
    variant: "preview-768",
    contentHash: "abc123",
  });

  // Without a control case a fully broken protocol and a healthy one are
  // indistinguishable: everything would simply report "no response".
  const control = cases.at(-1);
  assert.match(control.name, /control/);
  assert.match(control.note, /400/);
});

test("render state passes when images actually load", () => {
  const checks = evaluateRenderState({
    totalCards: 8,
    posterImgCount: 3,
    loadedOk: 3,
    broken: 0,
    sampleSrc: "http://thumbnail.localhost/pkg-1/preview-768/abc123",
  });

  assert.ok(checks.every((check) => check.pass));
  assert.equal(checks.length, 3);
});

test("render state fails when no image loaded", () => {
  const checks = evaluateRenderState({
    totalCards: 8,
    posterImgCount: 0,
    loadedOk: 0,
    broken: 0,
  });

  assert.equal(checks[1].pass, false);
  assert.match(checks[1].detail, /no image loaded/);
});

test("render state fails on broken images even when some loaded", () => {
  const checks = evaluateRenderState({
    totalCards: 8,
    posterImgCount: 3,
    loadedOk: 2,
    broken: 1,
  });

  assert.equal(checks[2].pass, false);
  assert.match(checks[2].detail, /broken=1/);
});

test("render state fails when the library has no cards at all", () => {
  const checks = evaluateRenderState({});
  assert.equal(checks[0].pass, false);
});

test("render state tolerates a null snapshot", () => {
  const checks = evaluateRenderState(null);
  assert.equal(checks.length, 3);
  assert.equal(checks.every((check) => check.pass), false);
});

test("network events are tallied by status", () => {
  const tally = summarizeNetworkEvents(["200", "200", "400", "FAILED(net::ERR_UNKNOWN_URL_SCHEME)"]);

  assert.deepEqual(tally, [
    { status: "200", count: 2 },
    { status: "400", count: 1 },
    { status: "FAILED(net::ERR_UNKNOWN_URL_SCHEME)", count: 1 },
  ]);
});

test("network tally stays empty without events", () => {
  assert.deepEqual(summarizeNetworkEvents([]), []);
  assert.deepEqual(summarizeNetworkEvents(null), []);
});
