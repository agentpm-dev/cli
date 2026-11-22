// Checkout SDK + real CLI. The auditor independently checks the saved requests.
import { pathToFileURL } from "node:url";

const [modulePath, cli, mode, agent] = process.argv.slice(2);
const { HarnessClient } = await import(pathToFileURL(modulePath).href);
const client = new HarnessClient({
  agentpmPath: cli,
  cwd: process.cwd(),
  agent: agent || undefined,
});
const requests = [];
if (mode === "run") {
  client.registerModelProvider("sdk-model", (payload) => {
    requests.push(payload.request);
    return {
      assistant_content: null,
      actions: [
        {
          id: `step-${requests.length}`,
          action:
            requests.length === 1
              ? { type: "agent_pm_tool", tool: "@fixture/echo", arguments: {} }
              : {
                  type: "phase_completion",
                  output: { text: "PASS M6B fixture" },
                },
        },
      ],
      finish_reason: "tool_calls",
    };
  });
}
try {
  const initialized = await client.initialize();
  const preflight = await client.preflight();
  const supported = await client.supportedStandards();
  let result = null;
  let blocked = null;
  if (mode === "run") result = await client.run("Check exact versions.");
  if (mode === "blocked") {
    try {
      await client.run("Must not execute.");
    } catch (error) {
      blocked = error.code;
    }
    if (blocked !== "preflight_not_ready")
      throw new Error(`Unexpected block: ${blocked}`);
  }
  await client.shutdown();
  console.log(
    JSON.stringify({
      sdk: "node",
      initialized,
      preflight,
      supported,
      result,
      blocked,
      requests,
    }),
  );
} finally {
  client.stop();
}
