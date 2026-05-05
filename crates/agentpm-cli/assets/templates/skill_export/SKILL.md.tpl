---
name: {{SKILL_NAME}}
description: {{SKILL_DESCRIPTION}}
---

# {{TITLE}}

## When to use this skill

- Use it when you want to run `{{PACKAGE_REF}}` from a skill-capable client while keeping execution delegated to AgentPM.
- Use it when the tool’s packaged runtime and manifest contract should stay the source of truth.
- TODO: Add the specific workflow cues that should trigger this skill in your environment.

## Quick start

Run the tool directly:

```bash
agentpm run {{PACKAGE_REF}} --input '{"example":true}'
```

Or use the helper script:

```bash
./scripts/run.sh '{"example":true}'
```

## What this skill covers

- Tool: `{{PACKAGE_REF}}`
- Installed version used for scaffold generation: `{{RESOLVED_VERSION}}`
- Description: {{DESCRIPTION}}

## References

- Tool contract and schema details: [references/tool-contract.md](references/tool-contract.md)
- Example invocations and adaptation ideas: [references/examples.md](references/examples.md)

## Workflow notes

- This skill is a generated starting point. Add workflow-specific guidance before relying on it broadly.
- Keep this file concise and workflow-oriented.
- Put deeper runtime/schema details in the reference files.
- TODO: Add step-by-step workflow guidance specific to your team or repo.
- TODO: Add examples of failure handling, retries, and escalation paths if this tool needs them.
