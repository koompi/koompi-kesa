# Operator Handoff Summary

- Status: clean
- Project: koompi_code_cli
- Branch: main
- Head: abc1234
- Generated: [GENERATED_AT]

## What Changed
- bd-63x3v.7.5: Rehearse extension crash quarantine and rollback

## Safe Next Actions
- Claim the next ready bead: bd-next.

## Must Not Touch
- No additional protected paths beyond repo instructions and active Beads ownership.

## Gates
- cargo_check_all_targets_rch: pass rch exec -- cargo check --all-targets

## Completion Audit
- source_present: false
- status: not_provided
- completion_allowed: none
- blocked_requirements: 0
- unresolved_gaps: 0

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
- completion_audit: pass - completion audit source=not_provided
