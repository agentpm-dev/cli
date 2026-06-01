# {{PROJECT_DISPLAY_NAME}}

This starter scaffold was generated from the `{{TEMPLATE_NAME}}` workflow template package.

When this template is used with `agentpm new`, files in the scaffold can reference render variables like `{{ project_name }}`.

## What to edit first

- This `README.md` should become the starter documentation that ships inside your template scaffold.
- Add the files you want `agentpm new` to copy or render for consumers under `template.files_root`.
- Use `{{ variable_name }}` placeholders in text files when you want `agentpm new` to render generation-time values.
- Do not include a root `agent.json` inside `template.files_root`. `agentpm new` synthesizes the generated project's root `agent.json` itself.
- If your template scaffolds additional local agent manifests, put them under `agents/` using files like `agents/reviewer.agent.json`. `agentpm new` treats `agents/` as the canonical location for extra local workspace agents.
- Add source files, `.env.example`, and any execution-surface-specific starter assets your template should provide.

## Security note

Template variables are for generation-time scaffold values only. Do not use them for API keys, tokens, passwords, or runtime secrets. Put runtime configuration in `.env.example` and manifest `environment.vars` where applicable.
