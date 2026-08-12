# Operator Handoff Summary

- Status: watch
- Project: koompi_code_cli
- Branch: main
- Head: abc1234
- Generated: [GENERATED_AT]

## What Changed
- No recently closed beads were provided.

## Safe Next Actions
- Inspect and preserve dirty worktree paths before editing overlapping files.

## Must Not Touch
- Do not overwrite dirty path without ownership: scripts/build_swarm_operator_runpack.py
- Do not overwrite dirty path without ownership: docs/contracts/new-contract.json

## Gates
- py_compile: pass

## Completion Audit
- source_present: false
- status: not_provided
- completion_allowed: none
- blocked_requirements: 0
- unresolved_gaps: 0

## Open Action-Plan Decisions
- None.

## Invariants
- git_worktree_clean: warn - 2 worktree path(s) need attention
- git_pushed: pass - HEAD matches upstream
- validation_gates: pass - validation status=pass
- evidence_freshness: pass - evidence freshness=fresh
- agent_mail_usable: pass - agent mail health=green semantic=pass
- reservations_current: pass - No expired reservations
- rch_available: pass - rch status=ok
- action_plan_decisions: pass - No open action-plan decisions
- completion_audit: pass - completion audit source=not_provided
