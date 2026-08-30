// Diagnoses whether the Tauri thumbnail custom protocol actually delivers images.
//
// Windows WebView2 cannot load a non-standard scheme, so the app rewrites
// `thumbnail://...` into `http://thumbnail.localhost/...` at the DTO boundary
// (see src-tauri/src/thumbnail_protocol.rs). When images stop showing up, this
// tells you which half of that chain is broken.
//
// Needs a dev chain with remote debugging enabled:
//   $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS="--remote-debugging-port=9223"
//   corepack pnpm tauri:dev
//
// Usage:
//   node scripts/check-thumbnail-protocol.mjs
//   node scripts/check-thumbnail-protocol.mjs --probe <packageId> <variant> <hash>
//   node scripts/check-thumbnail-protocol.mjs --port 9223
import { pathToFileURL } from "node:url";

export const DEFAULT_PORT = 9223;
export const CUSTOM_SCHEME = "thumbnail://";
export const WINDOWS_ORIGIN = "http://thumbnail.localhost/";

export function buildProbeCases({ packageId, variant, contentHash }) {
  const reference = `${packageId}/${variant}/${contentHash}`;
  return [
    {
      name: "custom scheme",
      url: `${CUSTOM_SCHEME}${reference}`,
      note: "rejected outright by WebView2 on Windows",
    },
    {
      name: "windows localhost origin",
      url: `${WINDOWS_ORIGIN}${reference}`,
      note: "what the app emits on Windows",
    },
    {
      name: "host carries package id",
      url: `http://thumbnail.${reference}`,
      note: "loads, but CSP cannot express this shape",
    },
    {
      name: "control (missing package)",
      url: `http://thumbnail.does-not-exist/${variant}/${contentHash}`,
      note: "must return 400 - proves the handler is reachable at all",
    },
  ];
}

export function evaluateRenderState(snapshot) {
  const {
    totalCards = 0,
    posterImgCount = 0,
    loadedOk = 0,
    broken = 0,
    sampleSrc = null,
  } = snapshot ?? {};

  return [
    {
      name: "library rendered cards",
      pass: totalCards > 0,
      detail: `cards=${totalCards}`,
    },
    {
      name: "at least one poster image loaded",
      pass: loadedOk > 0,
      detail: loadedOk > 0 ? `sample=${sampleSrc}` : "no image loaded",
    },
    {
      name: "no broken poster image",
      pass: broken === 0,
      detail: `posterImg=${posterImgCount} loaded=${loadedOk} broken=${broken}`,
    },
  ];
}

export function summarizeNetworkEvents(events) {
  const tally = new Map();
  for (const event of events ?? []) {
    tally.set(event, (tally.get(event) ?? 0) + 1);
  }
  return [...tally.entries()].map(([status, count]) => ({ status, count }));
}

const RENDER_EXPRESSION = `(() => {
  const imgs = Array.from(document.querySelectorAll('.mod-card__poster-img'));
  const loaded = imgs.filter((i) => i.complete && i.naturalWidth > 0);
  const broken = imgs.filter((i) => i.complete && i.naturalWidth === 0);
  return JSON.stringify({
    route: location.pathname + location.hash,
    totalCards: document.querySelectorAll('.mod-card').length,
    posterImgCount: imgs.length,
    loadedOk: loaded.length,
    broken: broken.length,
    sampleSrc: imgs.length ? imgs[0].getAttribute('src') : null,
  });
})()`;

function parseArgs(argv) {
  const portIndex = argv.indexOf("--port");
  const probeIndex = argv.indexOf("--probe");
  return {
    port: portIndex === -1 ? DEFAULT_PORT : Number(argv[portIndex + 1]),
    probe:
      probeIndex === -1
        ? null
        : {
            packageId: argv[probeIndex + 1],
            variant: argv[probeIndex + 2],
            contentHash: argv[probeIndex + 3],
          },
  };
}

async function connect(port) {
  let targets;
  try {
    targets = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json();
  } catch {
    return { error: `cannot reach CDP on port ${port}. Is the dev chain running with --remote-debugging-port=${port}?` };
  }

  const page = targets.find((target) => target.type === "page");
  if (!page) {
    return { error: `no page target. targets: ${targets.map((t) => `${t.type} ${t.url}`).join(", ")}` };
  }
  return { page };
}

async function main() {
  const { port, probe } = parseArgs(process.argv.slice(2));
  const connection = await connect(port);
  if (connection.error) {
    console.error(connection.error);
    return 1;
  }

  const { page } = connection;
  console.log(`target: ${page.url}`);

  const socket = new WebSocket(page.webSocketDebuggerUrl);
  let nextId = 1;
  const pending = new Map();
  const network = [];

  socket.addEventListener("message", (event) => {
    const message = JSON.parse(event.data);
    if (message.id && pending.has(message.id)) {
      pending.get(message.id)(message);
      pending.delete(message.id);
      return;
    }
    const url = message.params?.response?.url ?? message.params?.request?.url ?? "";
    if (!url.includes("thumbnail")) return;
    if (message.method === "Network.responseReceived") {
      network.push(`${message.params.response.status}`);
    } else if (message.method === "Network.loadingFailed") {
      network.push(`FAILED(${message.params.errorText})`);
    }
  });

  await new Promise((resolve) => socket.addEventListener("open", resolve, { once: true }));

  const send = (method, params = {}) =>
    new Promise((resolve) => {
      const id = nextId++;
      pending.set(id, resolve);
      socket.send(JSON.stringify({ id, method, params }));
    });

  await send("Network.enable");
  await send("Runtime.enable");
  await send("Page.enable");
  await send("Network.setCacheDisabled", { cacheDisabled: true });

  if (probe) {
    const cases = buildProbeCases(probe);
    const urls = JSON.stringify(cases.map((entry) => entry.url));
    const response = await send("Runtime.evaluate", {
      expression: `(async () => {
        const probeOne = (src) => new Promise((resolve) => {
          const img = new Image();
          const done = (r) => resolve(src + '  =>  ' + r);
          img.onload = () => done('LOADED ' + img.naturalWidth + 'x' + img.naturalHeight);
          img.onerror = () => done('ERROR');
          img.src = src;
          setTimeout(() => done('TIMEOUT (no event at all)'), 5000);
        });
        const out = [];
        for (const src of ${urls}) { out.push(await probeOne(src)); }
        return out.join('\\n');
      })()`,
      awaitPromise: true,
      returnByValue: true,
    });

    console.log("\n--- url shape probe ---");
    console.log(response.result?.result?.value ?? JSON.stringify(response.result));
    console.log("\nexpectation:");
    for (const entry of cases) {
      console.log(`  ${entry.name}: ${entry.note}`);
    }
  } else {
    // Navigate to the same URL rather than reloading: a plain reload drops the SPA
    // route and lands back on the dashboard, which reports zero cards.
    await send("Page.navigate", { url: page.url });

    let snapshot = null;
    for (let attempt = 0; attempt < 15; attempt += 1) {
      await new Promise((resolve) => setTimeout(resolve, 2000));
      const response = await send("Runtime.evaluate", {
        expression: RENDER_EXPRESSION,
        returnByValue: true,
      });
      try {
        snapshot = JSON.parse(response.result?.result?.value ?? "{}");
      } catch {
        snapshot = null;
      }
      if (snapshot && snapshot.totalCards > 0) break;
    }

    console.log("\n--- page state ---");
    console.log(JSON.stringify(snapshot, null, 2));

    const checks = evaluateRenderState(snapshot);
    for (const check of checks) {
      console.log(`${check.pass ? "PASS" : "FAIL"}  ${check.name}`);
      console.log(`      ${check.detail}`);
    }

    const events = summarizeNetworkEvents(network);
    console.log("\n--- thumbnail responses ---");
    console.log(
      events.length
        ? events.map((entry) => `  ${entry.status} x${entry.count}`).join("\n")
        : "  (none - no thumbnail request reached the network layer)",
    );

    socket.close();
    const failed = checks.filter((check) => !check.pass).length;
    return failed === 0 ? 0 : 1;
  }

  socket.close();
  return 0;
}

// Guarded so the pure helpers above can be imported by tests without the CDP
// client trying to connect.
const isDirectRun =
  process.argv[1] !== undefined &&
  import.meta.url === pathToFileURL(process.argv[1]).href;

if (isDirectRun) {
  process.exit(await main());
}
