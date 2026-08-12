{
  "$schema": "https://raw.githubusercontent.com/agentpm-dev/cli/refs/heads/main/schemas/agentpm.manifest.schema.json",
  "kind": "loop",
  "name": "{{LOOP_NAME}}",
  "version": "0.1.0",
  "description": "{{LOOP_DESCRIPTION}}",
  "readme": "README.md",
  "loop": {
    "archetype": "review_execute_escalate",
    "entry_phase": "assess",
    "limits": {
      "max_steps": 8
    },
    "phases": [
      {
        "id": "assess",
        "objective": "Assess the current task state and decide whether work can proceed safely.",
        "outcomes": [
          {
            "id": "proceed",
            "description": "The task can proceed to execution."
          },
          {
            "id": "handoff",
            "description": "The task should be handed off instead of executed here."
          }
        ]
      },
      {
        "id": "execute",
        "objective": "Perform the bounded unit of work and prepare the result for review."
      },
      {
        "id": "review",
        "objective": "Review the work and decide whether to finish or iterate once more.",
        "outcomes": [
          {
            "id": "needs-more-work",
            "description": "The work needs another execution pass before it can finish."
          },
          {
            "id": "ready",
            "description": "The work is ready to finish."
          }
        ]
      }
    ],
    "transitions": [
      {
        "from": "assess",
        "on": "proceed",
        "to": "execute"
      },
      {
        "from": "assess",
        "on": "handoff",
        "to": "$handoff"
      },
      {
        "from": "execute",
        "on": "complete",
        "to": "review"
      },
      {
        "from": "review",
        "on": "needs-more-work",
        "to": "execute"
      },
      {
        "from": "review",
        "on": "ready",
        "to": "$end"
      }
    ],
    "checkpoints": [
      {
        "id": "approve-execution",
        "type": "approval",
        "before_phase": "review",
        "on_reject": "$handoff"
      }
    ],
    "error_policy": {
      "tool_failure": {
        "action": "retry",
        "max_retries": 1,
        "on_exhausted": "fail_phase"
      },
      "phase_failure": {
        "action": "handoff"
      }
    }
  }
}
