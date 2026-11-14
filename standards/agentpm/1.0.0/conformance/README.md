# APDS 1.0.0 conformance evaluation

The structural checks in this directory evaluate manifest shape. They do not execute a Runner or establish full APDS 1.0.0 conformance on their own; an assessment also needs the applicable semantic checks and must state its supplied context.

Normative fixture expectations must agree with the schema and semantic rules. Resolve conflicts before publication; fixtures cannot override either document.

The nine [valid manifests](fixtures/valid/) cover all eight kinds and an Agent with no dependency fields. The Tool includes Python distribution requirements; the Template includes `agentpm-harness`. Referenced content is not supplied by this structural corpus, so file existence, generated build freshness and dependency resolution are not evaluated here.

[structural-cases.json](structural-cases.json) supplies identical description cases to independent Rust and Python JSON Schema tests. A description must contain at least one character outside the Unicode `White_Space` set: U+0009–000D, U+0020, U+0085, U+00A0, U+1680, U+2000–200A, U+2028–2029, U+202F, U+205F and U+3000. The explicit class in the schema avoids differences between language-specific `trim`, `strip` and regular-expression shorthand. U+200B and U+001C are not members of that set; the tests make this boundary explicit. Leading/trailing whitespace around meaningful text is allowed; validators do not rewrite descriptions.

Distinguish standalone (intrinsic) validation, resolved-composition validation and packaged-content validation. Missing context is not proof of either validity or invalidity for checks that require it. A declared standard, a structural pass, full conformance, Runner compatibility and provenance are separate claims.

Implementations can consume these JSON files without using AgentPM's code. The CLI repository currently checks them with its existing Rust Draft 2020-12 validator and Python `jsonschema` Draft 2020-12 validator. Diagnostic serialization is implementation-specific.

Before publishing a standard version, edit its canonical source and run `python3 scripts/check_apds_schema.py --write` from the CLI repository root after review. That command regenerates the old schema-path copy and checksum inventory; it does not edit the schema or choose semantic expectations. `python3 scripts/check_apds_schema.py` checks drift. Do not use synchronization to rewrite an already published standard version.
