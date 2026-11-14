# APDS 1.0.0 rule index

Each link identifies a rule in [semantics.md](../semantics.md). The rule text defines the requirement; this index locates its subjects, checks and related [semantic cases](semantic-cases.md). A case describes expected interpretation, not an additional requirement.

**Subjects:** A = author, V = validator, M = Manager, R = Runner. A Registry performing validation has V responsibilities. Consumers making support/verification claims also follow the scope and reporting rules.

**Checks:** S = standalone, C = resolved-composition, P = packaged-content, R = Runner interpretation. S/C/P checks establish applicable declaration validity; R checks observe execution or capability behavior. A validator need not execute a Runner to evaluate its own scope. Runtime-only rules do not manufacture invalid-manifest cases.

| Rule                                                                                    | Subjects   | Checks     | Related cases    |
| --------------------------------------------------------------------------------------- | ---------- | ---------- | ---------------- |
| [APDS-SCOPE-001 — Role-specific support](../semantics.md#apds-scope-001)                | A, V, M, R | S, C, P, R | N, P             |
| [APDS-SCOPE-002 — One versioned contract](../semantics.md#apds-scope-002)               | A, V       | S, C, P    | —                |
| [APDS-ID-001 — Explicit standard](../semantics.md#apds-id-001)                          | A, V       | S          | —                |
| [APDS-ID-002 — Artifact identity and purpose](../semantics.md#apds-id-002)              | A, V       | S          | H                |
| [APDS-ID-003 — Reference forms](../semantics.md#apds-id-003)                            | A, V, M    | S, C       | I                |
| [APDS-ID-004 — Content paths](../semantics.md#apds-id-004)                              | A, V, M, R | S, P, R    | N                |
| [APDS-GRAPH-001 — Allowed relationships](../semantics.md#apds-graph-001)                | A, V       | S, C       | H                |
| [APDS-GRAPH-002 — Selected versions](../semantics.md#apds-graph-002)                    | V, M, R    | C, R       | I, N             |
| [APDS-GRAPH-003 — Unambiguous references](../semantics.md#apds-graph-003)               | A, V, M, R | S, C, R    | I, J             |
| [APDS-BIND-001 — Declared membership](../semantics.md#apds-bind-001)                    | A, V       | S, C       | A, C, F, L       |
| [APDS-BIND-002 — Phase names](../semantics.md#apds-bind-002)                            | A, V       | S, C       | D, N             |
| [APDS-BIND-003 — Additive scopes](../semantics.md#apds-bind-003)                        | R          | C, R       | A, C, K, L, M    |
| [APDS-BIND-004 — Eligibility is not invocation](../semantics.md#apds-bind-004)          | R          | R          | B, G, P          |
| [APDS-BIND-005 — MCP and consumer context](../semantics.md#apds-bind-005)               | A, V, R    | S, C, P, R | B                |
| [APDS-SKILL-001 — Skill resources](../semantics.md#apds-skill-001)                      | A, V, R    | S, P, R    | —                |
| [APDS-SKILL-002 — Tool inheritance](../semantics.md#apds-skill-002)                     | R          | C, R       | A, B, C          |
| [APDS-SKILL-003 — Repeated paths](../semantics.md#apds-skill-003)                       | M, R       | C, R       | I, J             |
| [APDS-TOOL-001 — Process invocation](../semantics.md#apds-tool-001)                     | A, R       | R          | —                |
| [APDS-TOOL-002 — Entrypoint and runtime](../semantics.md#apds-tool-002)                 | A, V, R    | S, R       | P                |
| [APDS-TOOL-003 — Environment precedence](../semantics.md#apds-tool-003)                 | R          | R          | —                |
| [APDS-LOOP-001 — Phases and outcomes](../semantics.md#apds-loop-001)                    | A, V, R    | S, R       | O                |
| [APDS-LOOP-002 — Deterministic graph](../semantics.md#apds-loop-002)                    | A, V, R    | S, R       | D, O             |
| [APDS-LOOP-003 — Terminals](../semantics.md#apds-loop-003)                              | R          | R          | O                |
| [APDS-LOOP-004 — Step limits](../semantics.md#apds-loop-004)                            | R          | R          | —                |
| [APDS-LOOP-005 — Approval checkpoints](../semantics.md#apds-loop-005)                   | A, V, R    | S, R       | —                |
| [APDS-LOOP-006 — Explicit error policy](../semantics.md#apds-loop-006)                  | A, V, R    | S, R       | —                |
| [APDS-LOOP-007 — Access policy](../semantics.md#apds-loop-007)                          | R          | R          | A, B, E, F, L    |
| [APDS-PROFILE-001 — Authored guidance](../semantics.md#apds-profile-001)                | A, V, R    | S, R       | K                |
| [APDS-PROFILE-002 — Additive guidance](../semantics.md#apds-profile-002)                | R          | C, R       | K                |
| [APDS-PROFILE-003 — Compatibility expectations](../semantics.md#apds-profile-003)       | R          | R          | E, P             |
| [APDS-KNOW-001 — Modes and content](../semantics.md#apds-know-001)                      | A, V, R    | S, P, R    | P                |
| [APDS-KNOW-002 — Packaged relationships](../semantics.md#apds-know-002)                 | A, V       | P          | N                |
| [APDS-KNOW-003 — Embeddings and indexes](../semantics.md#apds-know-003)                 | A, V, R    | S, P, R    | P                |
| [APDS-KNOW-004 — Retrieval preferences](../semantics.md#apds-know-004)                  | R          | R          | P                |
| [APDS-KNOW-005 — Provenance and scope](../semantics.md#apds-know-005)                   | V, R       | C, P, R    | P                |
| [APDS-MEM-001 — Scopes and record contracts](../semantics.md#apds-mem-001)              | A, V, R    | S, C, R    | F, L             |
| [APDS-MEM-002 — Space models](../semantics.md#apds-mem-002)                             | R          | R          | P                |
| [APDS-MEM-003 — Retrieval](../semantics.md#apds-mem-003)                                | A, V, R    | S, R       | P                |
| [APDS-MEM-004 — Capacity, retention and durations](../semantics.md#apds-mem-004)        | A, V, R    | S, R       | P                |
| [APDS-MEM-005 — Append-only](../semantics.md#apds-mem-005)                              | A, V, R    | S, R       | —                |
| [APDS-MEM-006 — Packaged record schemas](../semantics.md#apds-mem-006)                  | A, V, R    | P, R       | N                |
| [APDS-MEM-007 — Operation kinds](../semantics.md#apds-mem-007)                          | A, V, R    | S, R       | F                |
| [APDS-MEM-008 — Triggers](../semantics.md#apds-mem-008)                                 | A, V, R    | S, R       | G, M, Q, R, S, T |
| [APDS-MEM-009 — Space access and operation participation](../semantics.md#apds-mem-009) | R          | C, R       | F, L, M          |
| [APDS-MEM-010 — Successful output and source handling](../semantics.md#apds-mem-010)    | A, V, R    | S, R       | F                |
| [APDS-MEM-011 — Direct access and lifecycle operations](../semantics.md#apds-mem-011)   | R          | R          | F, L, M          |
| [APDS-MEM-012 — Governance annotations](../semantics.md#apds-mem-012)                   | A, V, R    | P, R       | P                |
| [APDS-MEM-013 — Logical record mapping](../semantics.md#apds-mem-013)                   | R          | R          | P                |
| [APDS-TEMPLATE-001 — Files and variables](../semantics.md#apds-template-001)            | A, V, M    | S, P, R    | —                |
| [APDS-TEMPLATE-002 — Dependencies and entrypoints](../semantics.md#apds-template-002)   | A, V, M, R | C, R       | P                |
| [APDS-TEMPLATE-003 — Execution surfaces](../semantics.md#apds-template-003)             | M, R       | R          | P                |
| [APDS-CONF-001 — Evaluation scopes](../semantics.md#apds-conf-001)                      | V          | S, C, P    | N                |
| [APDS-CONF-002 — Absent context versus absent content](../semantics.md#apds-conf-002)   | V          | S, C, P    | N                |
| [APDS-CONF-003 — Logical outcomes](../semantics.md#apds-conf-003)                       | V, M, R    | S, C, P, R | N, P             |
| [APDS-CONF-004 — Independent evidence](../semantics.md#apds-conf-004)                   | V, M, R    | S, C, P, R | H, P             |
| [APDS-CONF-005 — Truthful reporting](../semantics.md#apds-conf-005)                     | V, M, R    | S, C, P, R | N, P             |

The [structural fixtures](fixtures/valid/) and [description cases](structural-cases.json) exercise schema expectations. The [semantic-case map](semantic-cases.md) separates resolved relationship checks from observable Runner behavior. Advisory defaults, descriptive hints and execution-only rules do not imply that a standalone manifest should be rejected.
