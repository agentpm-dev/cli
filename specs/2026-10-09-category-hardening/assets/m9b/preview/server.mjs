import { createRequire } from "node:module";
import { pathToFileURL } from "node:url";
import { writeFileSync } from "node:fs";
const web = process.cwd();
const webRequire = createRequire(web + "/package.json");
const require = createRequire(webRequire.resolve("vitest/package.json"));
const vite = await import(pathToFileURL(require.resolve("vite")));
const createServer = vite.createServer ?? vite.default.createServer;
const root = "/private/tmp/m9b-ui";
const server = await createServer({
  configFile: web + "/vitest.config.ts",
  root,
  cacheDir: root + "/cache",
  css: { postcss: web },
  resolve: {
    alias: {
      "@": web + "/src",
      react: web + "/node_modules/react",
      "react-dom": web + "/node_modules/react-dom",
    },
  },
  server: {
    host: "127.0.0.1",
    port: 3029,
    strictPort: true,
    fs: { allow: [web, root] },
  },
  esbuild: { jsx: "automatic" },
});
const { agentActions } = await server.ssrLoadModule(
  web + "/src/lib/agent-actions.ts",
);
const { inspectionFixture } = await server.ssrLoadModule(
  web + "/src/test-fixtures/m9a/inspection.ts",
);
const state = inspectionFixture();
state.agent.package_slug = "@fixture/consumer";
state.agent.version = "1.0.0";
const a = agentActions(state.agent, state.loop);
writeFileSync(
  root + "/actions.json",
  JSON.stringify({ ...a, runExample: a.runExample(0) }, null, 2),
);
await server.listen();
console.log("Isolated M9B component preview on http://127.0.0.1:3029");
