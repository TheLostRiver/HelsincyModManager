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

function stripRustComments(source) {
  let blockCommentDepth = 0;
  let inLineComment = false;
  let uncommentedSource = "";
  for (let index = 0; index < source.length; index += 1) {
    const pair = source.slice(index, index + 2);
    if (inLineComment) {
      if (source[index] === "\n") {
        inLineComment = false;
        uncommentedSource += "\n";
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
        uncommentedSource += "\n";
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
    uncommentedSource += source[index];
  }

  assert.equal(blockCommentDepth, 0, "Rust block comment is not closed");
  return uncommentedSource;
}

function extractHandlerBody(source) {
  const uncommentedSource = stripRustComments(source);
  const markerOffsets = [];
  let offset = uncommentedSource.indexOf(handlerMarker);
  while (offset !== -1) {
    markerOffsets.push(offset);
    offset = uncommentedSource.indexOf(
      handlerMarker,
      offset + handlerMarker.length,
    );
  }

  assert.equal(
    markerOffsets.length,
    1,
    "expected exactly one tauri::generate_handler! command registry",
  );

  const bodyStart = markerOffsets[0] + handlerMarker.length;
  let bracketDepth = 1;
  for (let index = bodyStart; index < uncommentedSource.length; index += 1) {
    if (uncommentedSource[index] === "[") {
      bracketDepth += 1;
    } else if (uncommentedSource[index] === "]") {
      bracketDepth -= 1;
      if (bracketDepth === 0) {
        return uncommentedSource.slice(bodyStart, index);
      }
    }
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

test("ignores Rust comments while locating and parsing the command registry", () => {
  const fixture = `
// tauri::generate_handler![line_comment_example]
/*
  tauri::generate_handler![
    block_comment_example,
  ]
*/
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

function extractFunctionBody(source, signature) {
  const start = source.indexOf(signature);
  assert.notEqual(start, -1, `missing ${signature}`);
  const bodyStart = source.indexOf("{", start);
  assert.notEqual(bodyStart, -1, `missing body for ${signature}`);
  let depth = 0;
  for (let index = bodyStart; index < source.length; index += 1) {
    const character = source[index];
    if (character === "{") {
      depth += 1;
    } else if (character === "}") {
      depth -= 1;
      if (depth === 0) {
        return source.slice(bodyStart, index + 1);
      }
    }
  }
  assert.fail(`unbalanced braces for ${signature}`);
}

function extractMappingValues(body) {
  return [...body.matchAll(/=>\s*"([a-z_]+)"/g)].map((match) => match[1]);
}

// 契约里 `install_failed:<phase>` 的枚举会悄悄过期——#284 R5 时代码实际发 15 种、
// 契约只登记 7 种。根因是 phase 有三个来源：调用点的字面量、
// `InstallWriteAdmissionError::failure_phase()`，以及
// `CrossProcessWriteAdmissionError::code()`。后两者**不是调用点字面量**，
// 只 grep 字面量会漏掉整整一族（那次漏了 4 个 `write_admission_*`）。
// 这条用例直接从两个映射函数里取值，要求契约逐个登记。
test("documents every install failure phase produced by the admission layers", () => {
  const installTaskSource = readFileSync(
    join(repositoryRoot, "src-tauri/crates/hmm-app/src/install_task.rs"),
    "utf8",
  );
  const writeAdmissionSource = readFileSync(
    join(repositoryRoot, "src-tauri/crates/hmm-ports/src/write_admission.rs"),
    "utf8",
  );

  const phases = [
    ...extractMappingValues(
      extractFunctionBody(installTaskSource, "fn failure_phase(&self) -> &'static str"),
    ),
    ...extractMappingValues(
      extractFunctionBody(writeAdmissionSource, "pub const fn code(self) -> &'static str"),
    ),
  ];

  assert.ok(phases.length > 0, "expected admission-derived failure phases");
  assert.ok(
    phases.includes("write_admission_busy"),
    "extraction must cover the cross-process admission codes",
  );

  const undocumented = phases.filter((phase) => !contractSource.includes(phase));
  assert.deepEqual(
    undocumented,
    [],
    `undocumented install failure phases: ${undocumented.join(", ")}`,
  );
});

// #286 adopt 是外部状态命令族里唯一有写入的一条，前端要按稳定码逐个映射文案
// （complete-set 用例会盯着每个 key），所以契约必须**逐个**登记而不是只写一个通配族。
// 三个来源：接管器 `code()`、任务服务 `code()`，以及 completed 事件上的显式降级码常量。
test("documents every external mod adopt error code the runtime can emit", () => {
  const adoptSource = readFileSync(
    join(repositoryRoot, "src-tauri/crates/hmm-runtime/src/external_mod_adopt.rs"),
    "utf8",
  );
  const adoptTasksSource = readFileSync(
    join(repositoryRoot, "src-tauri/crates/hmm-runtime/src/external_mod_adopt_tasks.rs"),
    "utf8",
  );

  const codes = [
    ...extractMappingValues(
      extractFunctionBody(adoptSource, "pub fn code(&self) -> &'static str"),
    ),
    ...extractMappingValues(
      extractFunctionBody(adoptTasksSource, "pub const fn code(self) -> &'static str"),
    ),
    ...[...adoptTasksSource.matchAll(/_CODE: &str = "([a-z_]+)"/g)].map((match) => match[1]),
  ];

  assert.ok(
    codes.includes("external_mod_adopt_stale") &&
      codes.includes("external_mod_adopt_task_unavailable") &&
      codes.includes("external_mod_adopt_audit_unavailable"),
    "extraction must cover the adopter, the task service and the degradation code",
  );

  const undocumented = codes.filter((code) => !contractSource.includes(code));
  assert.deepEqual(
    undocumented,
    [],
    `undocumented external mod adopt codes: ${undocumented.join(", ")}`,
  );
});

// 扫描族此前只在契约里写了一个通配 `external_state_scan_*`，新增一个变体不会有任何东西变红。
// 两个来源：扫描器 `code()` 与任务服务 `code()`（后者是 command 错误码 + runner 兜底终态）。
test("documents every external state scan error code the runtime can emit", () => {
  const scanSource = readFileSync(
    join(repositoryRoot, "src-tauri/crates/hmm-runtime/src/external_state_scan.rs"),
    "utf8",
  );
  const scanTasksSource = readFileSync(
    join(repositoryRoot, "src-tauri/crates/hmm-runtime/src/external_state_scan_tasks.rs"),
    "utf8",
  );

  const codes = [
    ...extractMappingValues(
      extractFunctionBody(scanSource, "pub const fn code(self) -> &'static str"),
    ),
    ...extractMappingValues(
      extractFunctionBody(scanTasksSource, "pub const fn code(self) -> &'static str"),
    ),
  ];

  assert.ok(
    codes.includes("external_state_scan_stale") &&
      codes.includes("external_state_scan_admission_order_violation") &&
      codes.includes("external_state_scan_task_unavailable"),
    "extraction must cover the scanner and the task service",
  );

  const undocumented = codes.filter((code) => !contractSource.includes(code));
  assert.deepEqual(
    undocumented,
    [],
    `undocumented external state scan codes: ${undocumented.join(", ")}`,
  );
});

// completed 事件上的显式降级码不是失败 phase，上面按 `failure_phase()` 取值的用例抓不到它；
// 三个任务服务各写一次字面量，任何一处改名都必须回到契约登记。
test("documents the audit degradation code carried by completed install events", () => {
  const sources = [
    "src-tauri/crates/hmm-app/src/install_task.rs",
    "src-tauri/crates/hmm-app/src/reinstall_task.rs",
  ].map((relative) => readFileSync(join(repositoryRoot, relative), "utf8"));

  const codes = [
    ...new Set(
      sources.flatMap((source) =>
        [...source.matchAll(/event\.error = Some\("([a-z_]+)"\.to_owned\(\)\)/g)].map(
          (match) => match[1],
        ),
      ),
    ),
  ];

  assert.deepEqual(
    codes,
    ["install_audit_unavailable"],
    "the completed-event degradation literal moved or gained siblings; update this extraction",
  );
  for (const code of codes) {
    assert.ok(contractSource.includes(code), `undocumented completed-event degradation code: ${code}`);
    for (const phase of ["install.completed", "install.uninstall.completed", "install.reinstall.completed"]) {
      assert.match(
        contractSource,
        new RegExp(`\\| \`install\` \\| \`${phase.replaceAll(".", "\\.")}\` \\|[^\\n]*${code}`),
        `${code} must be documented on the ${phase} phase-table row`,
      );
    }
  }
});
