# Operator Handoff Summary

- Status: blocked
- Project: koompi_code_cli
- Branch: main
- Head: abc1234
- Generated: [GENERATED_AT]

## What Changed
- No recently closed beads were provided.

## Safe Next Actions
- Resolve completion-audit blockers before admitting closeout.

## Must Not Touch
- Do not delete files from completion-audit claim-boundary evidence.

## Gates
- No validation gates were provided.

## Completion Audit
- source_present: true
- status: incomplete
- completion_allowed: false
- blocked_requirements: 1
- unresolved_gaps: 0
- proxy_only_requirements: 1

## Open Action-Plan Decisions
- None.

## Invariants
- git_worktree_clean: pass - Worktree is clean
- git_pushed: pass - HEAD matches upstream
- validation_gates: pass - validation status=pass
- evidence_freshness: pass - evidence freshness=fresh
- agent_mail_usable: pass - agent mail health=green semantic=pass
- reservations_current: pass - No expired reservations
- rch_available: pass - rch status=ok
- action_plan_decisions: pass - No open action-plan decisions
- completion_audit: block - completion audit status=incomplete blocked=1 gaps=0
