import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "node:test";

const currentDirectory = dirname(fileURLToPath(import.meta.url));
const repositoryRoot = resolve(currentDirectory, "../../..");
const tauriEntrySource = readFileSync(
  join(repositoryRoot, "src-tauri/src/lib.rs"),
  "utf8",
);
const contractSource = readFileSync(
  join(repositoryRoot, "docs/FRONTEND_BACKEND_CONTRACT.md"),
  "utf8",
);
const handlerMarker = "tauri::generate_handler![";

function extractHandlerBody(source) {
  const markerOffsets = [];
  let offset = source.indexOf(handlerMarker);
  while (offset !== -1) {
    markerOffsets.push(offset);
    offset = source.indexOf(handlerMarker, offset + handlerMarker.length);
  }

  assert.equal(
    markerOffsets.length,
    1,
    "expected exactly one tauri::generate_handler! command registry",
  );

  const bodyStart = markerOffsets[0] + handlerMarker.length;
  let bracketDepth = 1;
  let blockCommentDepth = 0;
  let inLineComment = false;
  let body = "";
  for (let index = bodyStart; index < source.length; index += 1) {
    const pair = source.slice(index, index + 2);
    if (inLineComment) {
      if (source[index] === "\n") {
        inLineComment = false;
        body += "\n";
      }
      continue;
    }
    if (blockCommentDepth > 0) {
      if (pair === "/*") {
        blockCommentDepth += 1;
        index += 1;
      } else if (pair === "*/") {
        blockCommentDepth -= 1;
        index += 1;
      } else if (source[index] === "\n") {
        body += "\n";
      }
      continue;
    }
    if (pair === "//") {
      inLineComment = true;
      index += 1;
      continue;
    }
    if (pair === "/*") {
      blockCommentDepth = 1;
      index += 1;
      continue;
    }
    if (source[index] === "[") {
      bracketDepth += 1;
    } else if (source[index] === "]") {
      bracketDepth -= 1;
      if (bracketDepth === 0) {
        return body;
      }
    }
    body += source[index];
  }

  assert.fail("tauri::generate_handler! command registry is not closed");
}

function extractRegisteredCommands(source) {
  const body = extractHandlerBody(source);
  const entries = body
    .split(",")
    .map((entry) => entry.trim())
    .filter(Boolean);

  return entries.map((entry) => {
    const match = entry.match(
      /^(?:[A-Za-z_][A-Za-z0-9_]*::)*([A-Za-z_][A-Za-z0-9_]*)$/,
    );
    assert.ok(match, `unexpected generate_handler entry: ${entry}`);
    return match[1];
  });
}

function contractContainsCommand(contract, command) {
  const escaped = command.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  return new RegExp(
    `(^|[^A-Za-z0-9_])${escaped}($|[^A-Za-z0-9_])`,
    "m",
  ).test(contract);
}

test("extracts commands without treating comment brackets as registry syntax", () => {
  const fixture = `
tauri::generate_handler![
  first,
  // This closing bracket is commentary: ]
  module::second,
  /* Nested comment syntax is valid in Rust: /* [ */ ] */
  third,
]`;

  assert.deepEqual(extractRegisteredCommands(fixture), [
    "first",
    "second",
    "third",
  ]);
});

test("extracts the unique registered Tauri commands from generate_handler", () => {
  const commands = extractRegisteredCommands(tauriEntrySource);

  assert.ok(commands.length > 0, "expected registered Tauri commands");
  assert.equal(
    new Set(commands).size,
    commands.length,
    "generate_handler contains duplicate command registrations",
  );
});

test("documents every registered Tauri command in the frontend/backend contract", () => {
  const commands = extractRegisteredCommands(tauriEntrySource);
  const missingCommands = commands.filter(
    (command) => !contractContainsCommand(contractSource, command),
  );

  assert.deepEqual(
    missingCommands,
    [],
    `undocumented registered Tauri commands: ${missingCommands.join(", ")}`,
  );
});
