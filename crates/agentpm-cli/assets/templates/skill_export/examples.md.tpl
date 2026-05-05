# Examples

These examples are generated starters. Adjust them to match the real workflow this skill should support.

## Inline JSON

```bash
agentpm run {{PACKAGE_REF}} --input '{{INLINE_INPUT}}'
```

## stdin JSON

```bash
cat <<'JSON' | agentpm run {{PACKAGE_REF}}
{{PRETTY_INPUT}}
JSON
```

## Helper script

```bash
./scripts/run.sh '{{INLINE_INPUT}}'
```

## TODOs

- TODO: Add one realistic example from your actual workflow.
- TODO: Add examples for invalid input or failure cases if this tool is safety-sensitive.
- TODO: Note any required environment variables or credentials that should be set before running the tool.
