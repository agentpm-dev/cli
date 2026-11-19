# Changelog

## 0.1.45 — Unreleased

- Bundle AgentPM APDS 1.0.0 with offline structural and semantic validation,
  stable rule IDs and versioned editor-schema links.
- Make `agentpm init` start with an Agent Package by default. All eight package
  kinds and generated Skill/root manifests declare their supported standard.
- Apply standalone APDS checks to ordinary lint and authoring/build/publication
  preflight. Explicit resolved and packaged scopes inspect supplied dependency
  graphs and package content; they do not establish Runner readiness.
- Add `agentpm lint --legacy` as a read-only compatibility preview for older
  undeclared sources. It neither changes files nor authorizes publication.
- Preserve historical installed artifacts while requiring an explicit supported
  standard and a nonblank description for newly authored publications.

### Updating an older local manifest

Add this declaration to your editable `agent.json`, provide a meaningful
description, and run `agentpm lint`:

```json
{ "standard": { "id": "agentpm", "version": "1.0.0" } }
```

Rebuild generated Knowledge/Memory content or saved package builds before
publishing. Empty or whitespace-only descriptions now fail validation. Custom
schema overrides cannot replace the bundled APDS contract for publication.
Previously published immutable versions do not need to be rewritten or re-signed.

CLI 0.1.36 remains the minimum version that reads and preserves the declaration;
0.1.45 introduces the authoring checks described here. Registry publication
enforcement is activated separately. SDK minimum versions and existing lockfile
formats are unchanged.

See [lint and migration guidance](https://www.agentpackagemanager.com/docs/v0.1/cli/lint#older-local-manifests)
and [publication requirements](https://www.agentpackagemanager.com/docs/v0.1/cli/publish).
