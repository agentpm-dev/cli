import React from "react";
import { createRoot } from "react-dom/client";
import AgentGetStarted from "@/components/packages/AgentGetStarted";
import AgentExamples from "@/components/packages/AgentExamples";
import {
  inspectionFixture,
  minimalInspection,
} from "@/test-fixtures/m9a/inspection";
import Before from "./Before";
import BeforeExamples from "./BeforeExamples";
import "@/styles/globals.css";
const mode = new URLSearchParams(location.search).get("mode") || "ready";
const state =
  mode === "no-loop" || mode === "empty"
    ? minimalInspection()
    : inspectionFixture();
state.agent.package_slug = "@fixture/review-agent";
state.agent.examples =
  mode === "empty"
    ? []
    : [
        {
          title: "Review incident",
          prompt: "Review the incident.\nState findings and next steps.",
        },
        {
          title: "Handoff",
          prompt: "Summarize the remaining risks for the next operator.",
        },
      ];
state.agent.bindings = { consumer_context: { file: "CONTEXT.md" } };
if (mode === "legacy") delete state.agent.manifest.standard;
if (mode === "unsupported")
  state.agent.manifest.standard = { id: "agentpm", version: "9.0.0" };
if (mode === "backend-unavailable")
  state.loop = {
    status: "unavailable",
    explanation: "Controlled unavailable graph",
  };
if (mode === "long")
  state.agent.package_slug =
    "@fixture/" + "long-package-name-".repeat(5) + "agent";
createRoot(document.getElementById("root")!).render(
  <main className="mx-auto min-w-0 max-w-7xl px-4 py-8 font-sans">
    <p className="mb-4 text-sm text-slate-400">
      Internal synthetic UI check; real production components and CSS, isolated
      from Next/auth/Registry. No model or Tool execution.
    </p>
    <h1 className="mb-6 break-words text-2xl">
      {state.agent.package_slug}@{state.agent.version}
    </h1>
    {mode === "before" ? (
      <>
        <Before {...state} />
        <BeforeExamples agent={state.agent} />
      </>
    ) : (
      <>
        <AgentGetStarted {...state} />
        <section className="mt-8 border-t border-slate-800 pt-6">
          <h2 className="mb-4 text-lg font-semibold">Examples</h2>
          <AgentExamples {...state} />
        </section>
      </>
    )}
  </main>,
);
