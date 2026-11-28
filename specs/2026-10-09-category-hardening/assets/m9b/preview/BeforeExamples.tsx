import React from "react";
import { Panel, PANEL_LAYERS } from "@/components/ui/Panel";
// Original default panel markup; unused Next Link import is omitted in this isolated baseline.
function ToolDetailPanel({ title, children }) {
  return (
    <Panel layer={PANEL_LAYERS.RAISED}>
      <div className="flex min-w-0 items-center justify-between gap-3 text-sm font-semibold mb-2">
        <h2 className="min-w-0 truncate text-sm font-semibold mb-2">{title}</h2>
      </div>
      {children}
    </Panel>
  );
}

import Badge from "@/components/ui/Badge";
export default function BeforeExamples({ agent }) {
  return (
    <ToolDetailPanel title="Examples">
      {agent.examples.length === 0 ? (
        <div className="text-sm text-slate-500">
          No examples published for this agent yet.
        </div>
      ) : (
        <div className="space-y-3">
          {agent.examples.map((example, index) => (
            <div
              key={`${example.title ?? "example"}-${index}`}
              className="rounded-xl border border-slate-800 bg-slate-950/40 p-3"
            >
              <div className="mb-2 flex items-center gap-2">
                <Badge label={`Example ${index + 1}`} tone="amber" />
                {example.title && (
                  <div className="text-sm font-medium text-slate-200">
                    {example.title}
                  </div>
                )}
              </div>
              <pre className="whitespace-pre-wrap break-words text-sm text-slate-300">
                {example.prompt}
              </pre>
            </div>
          ))}
        </div>
      )}
    </ToolDetailPanel>
  );
}
