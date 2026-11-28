// Optional T-WEB-12 verification against an already running local web app.
// Isolated Chrome profile; all requests except GET/HEAD are blocked.
import { spawn } from "node:child_process";
import { mkdtempSync, mkdirSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
const directory = process.argv[2];
if (!directory)
  throw new Error(
    "Usage: node scripts/check-agent-detail-browser.mjs <output-directory> [local-base-url]",
  );
const baseUrl = new URL(process.argv[3] ?? "http://localhost:3000");
if (!["localhost", "127.0.0.1", "[::1]"].includes(baseUrl.hostname))
  throw new Error("Use a local app; this check does not target production");
mkdirSync(directory, { recursive: true });
const blockedMethods = [];
let blockHydrationScripts = false;
const browser = spawn(
  process.env.CHROME_BIN ??
    "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
  [
    "--headless=new",
    "--disable-background-timer-throttling",
    "--disable-renderer-backgrounding",
    "--disable-backgrounding-occluded-windows",
    "--remote-debugging-pipe",
    `--user-data-dir=${mkdtempSync(join(tmpdir(), "m9a-browser-"))}`,
    "--no-first-run",
    "--no-default-browser-check",
    "--disable-background-networking",
    "--disable-gpu",
  ],
  { stdio: ["ignore", "ignore", "ignore", "pipe", "pipe"] },
);
let nextId = 0;
let buffer = "";
const pending = new Map();
const events = new Map();
browser.stdio[4].on("data", (chunk) => {
  buffer += chunk.toString();
  let end;
  while ((end = buffer.indexOf("\0")) >= 0) {
    const message = JSON.parse(buffer.slice(0, end));
    buffer = buffer.slice(end + 1);
    if (message.method === "Fetch.requestPaused") {
      const request = message.params;
      const method = request.request.method;
      const blockScript =
        blockHydrationScripts && request.resourceType === "Script";
      const operation =
        !blockScript && (method === "GET" || method === "HEAD")
          ? call(
              "Fetch.continueRequest",
              { requestId: request.requestId },
              message.sessionId,
            )
          : call(
              "Fetch.failRequest",
              { requestId: request.requestId, errorReason: "BlockedByClient" },
              message.sessionId,
            );
      if (method !== "GET" && method !== "HEAD") blockedMethods.push(method);
      operation.catch(() => {});
    }
    const request = pending.get(message.id);
    if (request) {
      pending.delete(message.id);
      clearTimeout(request.timer);
      message.error
        ? request.reject(new Error(message.error.message))
        : request.resolve(message.result);
    }
    const eventKey = `${message.sessionId}:${message.method}`;
    const event = events.get(eventKey);
    if (event) {
      events.delete(eventKey);
      clearTimeout(event.timer);
      event.resolve(message.params);
    }
  }
});
browser.on("error", (error) => {
  for (const request of pending.values()) {
    clearTimeout(request.timer);
    request.reject(error);
  }
  pending.clear();
});
function call(method, params = {}, sessionId) {
  return new Promise((resolve, reject) => {
    const id = ++nextId;
    const timer = setTimeout(() => {
      pending.delete(id);
      reject(new Error(`CDP timed out: ${method}`));
    }, 45000);
    pending.set(id, { resolve, reject, timer });
    browser.stdio[3].write(
      JSON.stringify({
        id,
        method,
        params,
        ...(sessionId ? { sessionId } : {}),
      }) + "\0",
    );
  });
}
function once(method, sessionId) {
  return new Promise((resolve, reject) => {
    const key = `${sessionId}:${method}`;
    events.set(key, {
      resolve,
      timer: setTimeout(() => {
        events.delete(key);
        reject(new Error(`CDP event timed out: ${method}`));
      }, 45000),
    });
  });
}
const results = [];
writeFileSync(
  join(directory, "report.json"),
  JSON.stringify({
    status: "incomplete",
    scope: "Local read-only browser check has started",
  }) + "\n",
);
try {
  const { targetId } = await call("Target.createTarget", {
    url: "about:blank",
  });
  const { sessionId } = await call("Target.attachToTarget", {
    targetId,
    flatten: true,
  });
  await call("Page.enable", {}, sessionId);
  await call("Page.bringToFront", {}, sessionId);
  await call(
    "Fetch.enable",
    { patterns: [{ urlPattern: "*", requestStage: "Request" }] },
    sessionId,
  );
  await call(
    "Emulation.setFocusEmulationEnabled",
    { enabled: true },
    sessionId,
  );
  const evaluate = async (expression) => {
    const r = await call(
      "Runtime.evaluate",
      {
        expression,
        returnByValue: true,
        awaitPromise: true,
        includeCommandLineAPI: true,
      },
      sessionId,
    );
    if (r.exceptionDetails)
      throw new Error(
        r.exceptionDetails.exception?.description ?? r.exceptionDetails.text,
      );
    return r.result.value;
  };
  // Poll from Node: Chrome's injected command-line API is scoped to each evaluation,
  // so retaining getEventListeners in an asynchronous page callback can hang.
  const poll = async (expression, timeout = 10000) => {
    const start = Date.now();
    while (Date.now() - start < timeout) {
      if (await evaluate(expression)) return;
      await new Promise((resolve) => setTimeout(resolve, 50));
    }
    throw new Error("Timed out waiting for " + expression);
  };
  const load = async (url, width) => {
    console.log("Checking", width, url);
    await call(
      "Emulation.setDeviceMetricsOverride",
      { width, height: 1100, deviceScaleFactor: 1, mobile: false },
      sessionId,
    );
    const loaded = once("Page.loadEventFired", sessionId);
    const navigation = await call("Page.navigate", { url }, sessionId);
    // Removing a hash can be same-document navigation. This helper promises a
    // fresh document and readiness checks, so explicitly reload in that case.
    if (!navigation.loaderId) await call("Page.reload", {}, sessionId);
    await loaded;
    await poll('Boolean(document.querySelector("h2"))', 30000);
  };
  const key = async (key, code, modifiers = 0) => {
    await call(
      "Input.dispatchKeyEvent",
      {
        type: "keyDown",
        key,
        code,
        modifiers,
        windowsVirtualKeyCode: {
          Enter: 13,
          Tab: 9,
          Escape: 27,
          KeyA: 65,
          KeyK: 75,
          Slash: 191,
        }[code],
        ...(key === "Enter" ? { text: "\r" } : {}),
        ...(code === "KeyA" && modifiers ? { commands: ["selectAll"] } : {}),
      },
      sessionId,
    );
    await call(
      "Input.dispatchKeyEvent",
      { type: "keyUp", key, code, modifiers },
      sessionId,
    );
  };
  const click = async (selector) => {
    const box = await evaluate(
      `(()=>{const r=document.querySelector(${JSON.stringify(selector)}).getBoundingClientRect();return{x:r.x+r.width/2,y:r.y+r.height/2}})()`,
    );
    await call(
      "Input.dispatchMouseEvent",
      { type: "mousePressed", button: "left", clickCount: 1, ...box },
      sessionId,
    );
    await call(
      "Input.dispatchMouseEvent",
      { type: "mouseReleased", button: "left", clickCount: 1, ...box },
      sessionId,
    );
  };
  const assert = (condition, message) => {
    if (!condition) throw new Error(message);
  };

  await call("Browser.grantPermissions", {
    origin: baseUrl.origin,
    permissions: ["clipboardReadWrite", "clipboardSanitizedWrite"],
  });
  const picture = async (name) => {
    const shot = await call(
      "Page.captureScreenshot",
      { format: "png" },
      sessionId,
    );
    writeFileSync(
      join(directory, name + ".png"),
      Buffer.from(shot.data, "base64"),
    );
  };
  for (const width of [1440, 390]) {
    await load(baseUrl + "/?mode=before", width);
    await picture("before-" + width);
  }
  if (!process.argv.includes("--before-only")) {
    for (const width of [1440, 768, 390, 320]) {
      await load(baseUrl + "/?mode=ready", width);
      await call(
        "Emulation.setEmulatedMedia",
        { features: [{ name: "prefers-reduced-motion", value: "reduce" }] },
        sessionId,
      );
      await evaluate(
        `(()=>{const real=navigator.clipboard.writeText.bind(navigator.clipboard);navigator.clipboard.writeText=async text=>{window.lastCopy=text;await real(text);window.copyCompleted=true;};})()`,
      );
      const metrics = await evaluate(
        `({canvas:document.documentElement.clientWidth,body:document.documentElement.scrollWidth,headings:[...document.querySelectorAll('h3')].map(e=>e.textContent),setups:[...document.querySelectorAll('summary')].filter(e=>e.textContent==='Runner setup & local preflight').length})`,
      );
      assert(metrics.body <= metrics.canvas, "Page overflow " + width);
      assert(
        metrics.setups === 2,
        "One shared setup per Get started and Examples; never per prompt",
      );
      await picture("get-started-" + width);
      // Real keyboard activation of the Python tab and Copy control.
      await evaluate(
        `document.querySelector('[aria-label="Load format"] button:last-child').focus()`,
      );
      await key("Enter", "Enter");
      await poll(
        `document.querySelector('[aria-label="Load format"] button:last-child').getAttribute('aria-pressed')==='true'`,
      );
      for (let step = 0; step < 3; step++) {
        await key("Tab", "Tab");
        if (
          await evaluate(
            `document.activeElement.getAttribute('aria-label')==='Copy Load'`,
          )
        )
          break;
      }
      assert(
        await evaluate(
          `document.activeElement.getAttribute('aria-label')==='Copy Load'`,
        ),
        "SDK keyboard order (including scrollable code)",
      );
      await key("Enter", "Enter");
      await poll("window.copyCompleted===true");
      assert(
        (await evaluate("window.lastCopy")).includes(
          'load_agent("@fixture/review-agent@1.0.0")',
        ),
        "Exact Python copy",
      );
      // Native disclosure key action, not a DOM click simulation.
      await evaluate(
        `(()=>{const e=[...document.querySelectorAll('article summary')][0];e.scrollIntoView({block:'center'});e.focus();})()`,
      );
      await key("Enter", "Enter");
      assert(
        await evaluate(`document.querySelector('article details').open`),
        "Example expands via Enter",
      );
      await evaluate(
        `window.copyCompleted=false;document.querySelector('[aria-label="Copy example 1 command"]').focus()`,
      );
      await key("Enter", "Enter");
      await poll("window.copyCompleted===true");
      assert(
        (await evaluate("window.lastCopy")) ===
          "agentpm harness @fixture/review-agent@1.0.0 --headless --input-file prompt-1.txt",
        "Example input filename and exact selected version",
      );
      await evaluate(
        `document.querySelector('article').scrollIntoView({block:'start'})`,
      );
      await picture("examples-" + width);
      await evaluate(
        `(()=>{const e=[...document.querySelectorAll('summary')].find(e=>e.textContent==='Runner setup & local preflight');e.scrollIntoView({block:'center'});e.focus()})()`,
      );
      await key("Enter", "Enter");
      assert(
        await evaluate(
          `document.querySelector('[aria-label="Copy Local preflight"]').getBoundingClientRect().height>0`,
        ),
        "Shared preflight visible after native disclosure",
      );
      assert(
        await evaluate(
          `document.documentElement.scrollWidth<=document.documentElement.clientWidth`,
        ),
        "Expanded setup overflow",
      );
      results.push({
        mode: "ready",
        width,
        ...metrics,
        keyboard: true,
        clipboard: true,
      });
    }
    for (const mode of [
      "no-loop",
      "unsupported",
      "backend-unavailable",
      "legacy",
      "empty",
      "long",
    ]) {
      await load(baseUrl + "/?mode=" + mode, 390);
      const state = await evaluate(
        `({body:document.documentElement.scrollWidth,canvas:document.documentElement.clientWidth,run:!!document.querySelector('[aria-label="Copy Harness"]'),prompts:[...document.querySelectorAll('button[aria-label^="Copy prompt"]')].length,exampleRuns:[...document.querySelectorAll('article summary')].filter(e=>e.textContent==='Run this prompt with Harness').length})`,
      );
      assert(state.body <= state.canvas, "Negative-state overflow " + mode);
      if (["no-loop", "unsupported", "backend-unavailable"].includes(mode))
        assert(
          !state.run && state.exampleRuns === 0,
          "No run action when unavailable: " + mode,
        );
      if (mode === "empty")
        assert(state.prompts === 0, "No invented prompt in empty state");
      if (mode === "legacy")
        assert(state.run, "Qualified legacy interpretation remains usable");
      await picture(mode + "-390");
      results.push({ mode, width: 390, ...state });
    }
  }
  writeFileSync(
    join(directory, "report.json"),
    JSON.stringify(
      {
        status: "pass",
        scope:
          "Isolated hydrated production Get Started and Examples components with production CSS; synthetic props; no Next/auth/Registry verification",
        blockedMethods,
        results,
      },
      null,
      2,
    ) + "\n",
  );
  console.log("PASS responsive, keyboard, copy and qualified states");
} catch (error) {
  writeFileSync(
    join(directory, "report.json"),
    JSON.stringify(
      { status: "failed", error: String(error), results, blockedMethods },
      null,
      2,
    ) + "\n",
  );
  throw error;
} finally {
  await call("Browser.close").catch(() => browser.kill());
}
