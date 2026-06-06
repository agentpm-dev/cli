# Review Checklist

## Contract surfaces
- [ ] Confirm `agent.json` remains the manifest file for tools, agents, and templates.
- [ ] Confirm `kind: "template"` was added intentionally and consistently across schema, CLI, backend, and UI.
- [ ] Confirm existing `kind: "tool"` manifests remain backward compatible.
- [ ] Confirm existing `kind: "agent"` manifests remain backward compatible.
- [ ] Confirm `kind: "agent"` did not gain recursive `agents` dependencies.
- [ ] Confirm generated root `agent.json` files do not contain `agents` arrays.
- [ ] Confirm template-specific fields live under a top-level `template` object.
- [ ] Confirm public API responses for publish/install/search remain compatible for existing tool and agent clients.
- [ ] Confirm publish receipts for templates do not incorrectly route users to tool pages.
- [ ] Confirm docs were updated for any user-facing CLI, schema, API, or registry behavior.

## Correctness
- [ ] Check the main happy path: publish template → registry displays template → `agentpm new` generates project → dependencies install → generated project runs through intended execution surface.
- [ ] Check `agentpm new` refuses non-template packages.
- [ ] Check `agentpm new` validates the downloaded template manifest before writing files.
- [ ] Check `agentpm new` fails safely if the target directory exists and is non-empty.
- [ ] Check `agentpm new` safely handles missing required variables.
- [ ] Check `agentpm new` renders variables only where intended.
- [ ] Check binary or non-text files are not corrupted by rendering.
- [ ] Check template file copy/extraction cannot escape the target directory.
- [ ] Check generated root `agent.json` is schema-valid.
- [ ] Check generated `agent.lock` represents runnable dependencies, not a permanent dependency on the template artifact.
- [ ] Check template-declared tool dependencies are installed.
- [ ] Check template-declared agent dependencies are installed as workspace/root dependencies.
- [ ] Check installed agent package roots still pull in their tool dependencies.
- [ ] Check templates themselves are not recursively expanded by normal install graph resolution.
- [ ] Check next-step commands are printed but not executed.

## Security and safety
- [ ] Confirm `agentpm new` does not execute template-provided source files.
- [ ] Confirm `agentpm new` does not execute shell scripts from the template.
- [ ] Confirm `agentpm new` does not run package-manager commands such as `npm install` or `pip install`.
- [ ] Confirm `agentpm new` does not implement hidden lifecycle hooks.
- [ ] Confirm any future hook-related code paths are absent or explicitly disabled.
- [ ] Confirm artifact integrity is checked before template files are used.
- [ ] Confirm path traversal protections apply to template artifact extraction/copying.
- [ ] Confirm error messages do not leak secrets or presigned URLs unnecessarily.

## Regressions
- [ ] Check publishing a tool still works.
- [ ] Check publishing an agent still works.
- [ ] Check installing a tool still works.
- [ ] Check installing an agent still works.
- [ ] Check agent install still resolves transitive tools.
- [ ] Check `agentpm run` still works for existing locked tools.
- [ ] Check `agentpm serve --mcp` still exposes locked tools.
- [ ] Check `agentpm export --skill` still works.
- [ ] Check tool search and detail pages still work.
- [ ] Check agent search and detail pages still work.
- [ ] Check package signing/attestation behavior did not regress for tools or agents.
- [ ] Check scan status display did not regress for tools or agents.

## Tests and verification
- [ ] Confirm work was verified according to `test-plan.md`.
- [ ] Confirm schema tests cover valid and invalid templates.
- [ ] Confirm tests cover rejection of recursive `agents` dependencies on normal agent manifests.
- [ ] Confirm backend tests cover template publish permissions.
- [ ] Confirm backend tests cover kind conflict behavior.
- [ ] Confirm backend tests cover install graph behavior for templates versus agents.
- [ ] Confirm CLI tests cover `agentpm new` generation behavior.
- [ ] Confirm CLI tests cover no template code execution during generation.
- [ ] Confirm UI tests or manual screenshots cover template search/detail behavior.
- [ ] Confirm examples were linted/validated.
- [ ] Confirm at least one example was manually bootstrapped end-to-end.

## Pattern adherence
- [ ] Check existing package publish/install infrastructure was reused where practical.
- [ ] Check template support did not introduce a parallel package system unnecessarily.
- [ ] Check existing lockfile writer/reader patterns were reused where practical.
- [ ] Check existing CLI command style and output style were followed.
- [ ] Check existing schema style was followed.
- [ ] Check existing registry page/component patterns were reused where practical.
- [ ] Check any new abstractions are justified by the spec.
- [ ] Check no new dependency was added unless necessary.
- [ ] Check examples follow existing `agentpm-examples` conventions.

## Official examples
- [ ] Confirm `research-assistant-python` exists and demonstrates the Python SDK surface.
- [ ] Confirm `node-triage-worker` exists and demonstrates the Node SDK surface.
- [ ] Confirm `cli-automation-worker` exists and demonstrates `agentpm run`.
- [ ] Confirm `cli-automation-worker` uses `--input-file` or stdin rather than fragile inline JSON escaping.
- [ ] Confirm `mcp-tool-server` exists and demonstrates `agentpm serve --mcp`.
- [ ] Confirm `mcp-tool-server` docs mention HTTP MCP and do not imply stdio support.
- [ ] Confirm `multi-agent-support-workspace` exists.
- [ ] Confirm `multi-agent-support-workspace` installs multiple agent package roots through template dependencies.
- [ ] Confirm `multi-agent-support-workspace` docs clearly say agents do not recursively depend on agents.
- [ ] Confirm all official templates have README files with publish, bootstrap, and run instructions.
- [ ] Confirm all official templates include `.env.example` where environment variables are needed.

## Notes for reviewer
- Pay special attention to any place where `kind` was previously assumed to be only `tool` or `agent`.
- Pay special attention to any code path that defaults unknown kinds to `tool`; this may be compatibility behavior in some places but wrong for template-aware paths.
- Pay special attention to install graph expansion. Agents may expand tools; templates must not expand dependencies through normal install resolution.
- Pay special attention to generated project lockfiles. The generated project should lock runnable dependencies, not become permanently tied to the template artifact.
- Pay special attention to the security boundary. Copying code is allowed; executing template code during scaffolding is not.
- Pay special attention to UI wording. Templates should not be described as tools.
- Pay special attention to multi-agent examples. They are workspace scaffolds, not proof that AgentPM has recursive multi-agent orchestration.
