# D13: Agent description prompt comparison

Builder decision: **defer injection and keep current prompt behavior**. The user confirmed independent
Claude review passed with no remaining findings on **2026-11-21**. No APDS rule or production prompt assembly changed.

## Options and evidence boundary

The options are to inject every top-level Agent description, inject it only under
an optional setting, or retain current behavior until there is a demonstrated
benefit. An optional setting would add a product contract before establishing its
value. The selected option retains current behavior.

The reproducible comparison uses the actual Harness Engine and prompt assembler,
with a test-only model adapter that either forwards the request unchanged or adds
an `Agent description (experiment)` section. It executes a one-phase fixture and a
three-phase incident-review fixture. Both declare the same task in their phase
objectives; the multi-phase fixture also has a global Profile with evidence and
communication guidance. Both arms receive identical scripted phase completions.
The experiment records full rendered prompts, phase outputs, terminal status,
UTF-8 bytes and whitespace word counts.

This establishes prompt shape, traceability, duplication and control-flow/output
equivalence. A scripted model **cannot establish LLM fidelity or phase-output
quality improvements**. Provider tokens and cost are explicitly `null`, not
invented measurements. There are no paid requests. Byte/word counts are not token
estimates or provider billing evidence.

## Observed results

| Fixture                                           | Baseline prompt bytes / words | With description bytes / words | Output / terminal result           |
| ------------------------------------------------- | ----------------------------- | ------------------------------ | ---------------------------------- |
| One phase                                         | 1,409 / 194                   | 1,496 / 204                    | Same output; ended                 |
| Three phases with Profile, summed across requests | 7,454 / 968                   | 7,715 / 998                    | Same phase/terminal outputs; ended |

The additional section costs **87 bytes and 10 whitespace words per request** in
these fixtures. It repeats intent already present in the phase objective. The
Profile and its evidence constraint remain present once in each multi-phase
request in both arms. The test does not establish how a real model would resolve
contradictory natural-language guidance, and does not prescribe prompt order or
Profile conflict precedence.

There is no measured quality benefit sufficient to justify automatic injection.
Keep current behavior, with a regression asserting that the baseline has no
added description section. A future model-backed comparison can reconsider this
implementation choice without changing APDS. No real-model benefit is claimed.

## Reproduce

From the CLI repository root:

```bash
AGENTPM_DESCRIPTION_AB_RESULTS="$PWD/target/m6b-description-ab.json" \
  cargo test -p agentpm-cli description_prompt_ab -- --nocapture
```

The experiment source is
`crates/agentpm-cli/src/harness_engine/tests/description_ab.rs`.
`target/m6b-description-ab.json` is generated, gitignored inspection output. The
absolute output path avoids Cargo's per-package working directory. This decision,
the test and reproduction command are checked in; generated reports are not.
