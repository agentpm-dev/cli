# Tool Contract

## Identity

- Package ref: `{{PACKAGE_REF}}`
- Resolved version: `{{RESOLVED_VERSION}}`
- Manifest name: `{{MANIFEST_NAME}}`
- Manifest version: `{{MANIFEST_VERSION}}`
- Manifest description: {{DESCRIPTION}}

## Environment requirements

{{ENVIRONMENT}}

## Input schema

```json
{{INPUT_SCHEMA}}
```

## Output schema

```json
{{OUTPUT_SCHEMA}}
```

## Runtime metadata

This is reference/debugging context. In normal use, `agentpm run` should hide these details.

- Runtime: `{{RUNTIME}}`
- Entrypoint command: `{{ENTRYPOINT_COMMAND}}`
- Entrypoint args: `{{ENTRYPOINT_ARGS}}`
- Entrypoint cwd: `{{ENTRYPOINT_CWD}}`
- Timeout (ms): `{{TIMEOUT_MS}}`
