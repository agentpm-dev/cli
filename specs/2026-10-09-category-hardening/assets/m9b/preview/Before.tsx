import React from "react";
import AgentSectionIcon from "@/components/packages/AgentSectionIcon";
import CodeBox from "@/components/ui/CodeBox";
export default function Before({ agent, loop }) {
  const inspection = { loop };
  const loadOptions = [
    {
      name: "Node",
      code: `import { loadAgent } from '@agentpm/sdk';\nconst agent = await loadAgent('${agent.package_slug}@${agent.version}');`,
    },
    {
      name: "Python",
      code: `from agentpm import load_agent\nagent = load_agent("${agent.package_slug}@${agent.version}")`,
    },
  ];

  const getStarted = (
    <section
      aria-label="Get started"
      className="mb-6 border-t border-slate-800 pb-5 pt-6"
    >
      <div className="mb-5 flex items-start gap-3">
        <span className="rounded-xl border border-sky-700/50 bg-sky-500/15 p-2 text-sky-300">
          <AgentSectionIcon section="start" />
        </span>
        <div>
          <h2 className="text-lg font-semibold">Get started</h2>
          <p className="mt-1 text-sm text-slate-300">
            Install, run, or load this Agent Package.
          </p>
        </div>
      </div>
      <div className="grid min-w-0 gap-6 lg:grid-cols-3">
        <div className="min-w-0 border-l-2 border-sky-500/70 pl-4">
          <h3 className="text-sm font-semibold">
            <span className="mr-2 font-mono text-xs text-sky-300">01</span>
            Install
          </h3>
          <p className="mb-3 mt-1 text-sm text-slate-300">
            Resolve this version and its dependencies with the CLI.
          </p>
          <CodeBox
            inline
            title="Install"
            id="install-agent"
            options={[
              {
                name: "Install",
                code: `agentpm install ${agent.package_slug}@${agent.version}`,
              },
            ]}
          />
        </div>
        <div className="min-w-0 border-l-2 border-lime-500/65 pl-4">
          <h3 className="flex flex-wrap items-center gap-2 text-sm font-semibold">
            <span className="font-mono text-xs text-lime-300">02</span>
            {inspection.loop.status === "available"
              ? "Run with Harness"
              : "Check Harness readiness"}
            <span className="text-xs font-normal text-slate-500">
              {inspection.loop.status === "available"
                ? "Loop + model required"
                : "Preflight only"}
            </span>
          </h3>
          <p className="mb-3 mt-1 text-sm text-slate-300">
            {inspection.loop.status === "absent"
              ? "An Agent Package may omit a Loop; Harness requires a compatible Loop."
              : inspection.loop.status === "available"
                ? "Run the installed Agent with a configured Runner."
                : "Inspect Loop and bindings without starting a Run."}
          </p>
          <CodeBox
            inline
            title={
              inspection.loop.status === "available"
                ? "Harness"
                : "Harness preflight"
            }
            id="run-agent"
            options={[
              {
                name: inspection.loop.status === "available" ? "Run" : "Check",
                code: `agentpm harness ${agent.package_slug}@${agent.version}${inspection.loop.status === "available" ? "" : " --json"}`,
              },
            ]}
          />
          {agent.bindings?.consumer_context?.file && (
            <p className="mt-3 break-words text-sm leading-relaxed text-slate-400">
              <span className="font-medium text-amber-200">You supply: </span>
              <code className="text-amber-200">
                {agent.bindings.consumer_context.file}
              </code>
              <span className="block">
                Consumer-owned context; not a capability grant.
              </span>
            </p>
          )}
        </div>
        <div className="min-w-0 border-l-2 border-slate-600 pl-4">
          <h3 className="text-sm font-semibold">
            <span className="mr-2 font-mono text-xs text-slate-400">OR</span>
            Load via SDK
          </h3>
          <p className="mb-3 mt-1 text-sm text-slate-300">
            Consume the Agent’s resolved definition from an application.
          </p>
          <CodeBox inline title="Load" id="load-agent" options={loadOptions} />
        </div>
      </div>
    </section>
  );

  return getStarted;
}
