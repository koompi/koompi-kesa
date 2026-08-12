# Crew Architecture

## Simplified Workflow

```
PRD → plan → tasks → work → done
```

No epics. Just PRD-based task planning and execution.

## Orchestration Flow

```
┌─────────────────────────────────────────────────────────────────────────────────┐
│                           CREW ORCHESTRATION FLOW                                │
└─────────────────────────────────────────────────────────────────────────────────┘

  ┌─────────────────┐
  │   PRD / Spec    │
  │   (PRD.md, etc) │
  └────────┬────────┘
           │
           ▼
┌─────────────────────────────────────────────────────────────────────────────────┐
│  PHASE 1: PLANNING                                            pi_messenger({   │
│                                                                action: "plan"}) │
│                                                                                  │
│  ┌─────────────────────────────────────────────────────────────────────────┐   │
│  │                          PLANNER (opus)                                  │   │
│  │                                                                          │   │
│  │   1. Explore codebase (structure, patterns, conventions)                 │   │
│  │   2. Read project documentation                                          │   │
│  │   3. Web/GitHub research (if relevant)                                   │   │
│  │   4. Gap analysis (edge cases, security, testing)                        │   │
│  │   5. Task breakdown with dependencies                                    │   │
│  │   6. Append findings to planning-progress.md                             │   │
│  │   7. Reviewer checks plan; refine until SHIP or max passes               │   │
│  │                                                                          │   │
│  └──────────────────────────────┬──────────────────────────────────────────┘   │
│                                 │                                               │
│                                 ▼                                               │
│  ┌─────────────────────────────────────────────────────────────────────────┐   │
│  │                           TASKS CREATED                                  │   │
│  │                                                                          │   │
│  │   task-1: Setup types ──────────────────┐                               │   │
│  │   task-2: Core logic ───────────────────┼─── depends on task-1          │   │
│  │   task-3: API endpoints ────────────────┘                               │   │
│  │   task-4: Tests ──────────────────────────── depends on task-2, task-3  │   │
│  │                                                                          │   │
│  └─────────────────────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────────────────────┘
                                  │
                                  ▼
┌─────────────────────────────────────────────────────────────────────────────────┐
│  PHASE 2: WORK EXECUTION                                      pi_messenger({   │
│                                                                action: "work"}) │
│                                                                                  │
│   Ready tasks ──►  ┌────────────────────────────────────────────────────────┐   │
│   (deps met)       │                 WORKERS (parallel)                      │   │
│                    │                                                         │   │
│                    │   ┌─────────────┐    ┌─────────────┐                   │   │
│                    │   │   worker    │    │   worker    │   concurrency: 2  │   │
│                    │   │   (opus)    │    │   (opus)    │                   │   │
│                    │   │             │    │             │                   │   │
│                    │   │  task-1     │    │  task-3     │                   │   │
│                    │   └──────┬──────┘    └──────┬──────┘                   │   │
│                    │          │                  │                          │   │
│                    └──────────┼──────────────────┼──────────────────────────┘   │
│                               │                  │                              │
│                               ▼                  ▼                              │
│                         ┌──────────┐       ┌──────────┐                        │
│                         │   Done   │       │   Done   │                        │
│                         └────┬─────┘       └────┬─────┘                        │
│                              │                  │                               │
│                              └────────┬─────────┘                               │
│                                       │                                         │
│                                       ▼                                         │
│  ┌─────────────────────────────────────────────────────────────────────────┐   │
│  │                         REVIEW (per task)                                │   │
│  │                                                                          │   │
│  │                      ┌─────────────────────┐                             │   │
│  │                      │      reviewer       │                             │   │
│  │                      │   (gpt-5.2-high)    │                             │   │
│  │                      │                     │                             │   │
│  │                      │ Code quality, docs, │                             │   │
│  │                      │ correctness, style  │                             │   │
│  │                      └──────────┬──────────┘                             │   │
│  │                                 │                                        │   │
│  │                                 ▼                                        │   │
│  │          ┌──────────────────────────────────────────────┐               │   │
│  │          │  SHIP ✅  │  NEEDS_WORK 🔄  │  MAJOR_RETHINK ❌ │               │   │
│  │          └──────────────────────────────────────────────┘               │   │
│  │                                                                          │   │
│  └─────────────────────────────────────────────────────────────────────────┘   │
│                                                                                  │
│   ┌─────────────────────────────────────────────────────────────────────────┐   │
│   │  AUTONOMOUS MODE (autonomous: true)                                      │   │
│   │                                                                          │   │
│   │   Wave 1 ──► Wave 2 ──► Wave 3 ──► ... ──► All done or blocked          │   │
│   │                                                                          │   │
│   │   Continues until:                                                       │   │
│   │   • All tasks completed                                                  │   │
│   │   • All remaining tasks blocked (no ready tasks)                         │   │
│   └─────────────────────────────────────────────────────────────────────────┘   │
│                                                                                  │
└─────────────────────────────────────────────────────────────────────────────────┘
```

**Note:** Plan review is automatic inside the planning loop; implementation review is a separate manual action (`action: "review"`).

## Model Summary

| Role | Model | Agents |
|------|-------|--------|
| Planner | `claude-opus-4-5` | planner |
| Analyst | `claude-opus-4-5` | interview-generator, plan-sync |
| Worker | `claude-opus-4-5` | worker |
| Reviewer | `openai/gpt-5.2-high` | reviewer |

## Agent Inventory

### Planner (1)

| Agent | Model | Purpose |
|-------|-------|---------|
| `crew-planner` | opus | Analyzes codebase and PRD, produces task breakdown |

### Analysts (2)

| Agent | Model | Purpose |
|-------|-------|---------|
| `crew-interview-generator` | opus | Generates clarifying questions |
| `crew-plan-sync` | opus | Updates downstream task specs after changes |

### Workers (1)

| Agent | Model | Purpose |
|-------|-------|---------|
| `crew-worker` | opus | Implements tasks, writes code |

### Reviewers (1)

| Agent | Model | Purpose |
|-------|-------|---------|
| `crew-reviewer` | gpt-5.2-high | Code review with SHIP/NEEDS_WORK/MAJOR_RETHINK verdicts |

## Data Storage

```
.kode/messenger/crew/
├── plan.json                 # Plan metadata (PRD path, task counts)
├── plan.md                   # Planner output (task breakdown)
├── planning-progress.md      # Planning loop history + reviewer feedback
├── tasks/
│   ├── task-1.json           # Task metadata (status, deps, attempts)
│   ├── task-1.md             # Task specification
│   ├── task-2.json
│   └── task-2.md
├── blocks/
│   └── task-3.md             # Block context (if blocked)
├── artifacts/                # Debug artifacts (flat, auto-cleaned)
│   ├── {runId}_{agent}_input.md
│   ├── {runId}_{agent}_output.md
│   ├── {runId}_{agent}.jsonl
│   └── {runId}_{agent}_meta.json
└── config.json               # Project-level crew config
```

## Configuration

```json
{
  "crew": {
    "concurrency": {
      "workers": 2
    },
    "truncation": {
      "planners": { "bytes": 204800, "lines": 5000 },
      "workers": { "bytes": 204800, "lines": 5000 },
      "reviewers": { "bytes": 102400, "lines": 2000 },
      "analysts": { "bytes": 102400, "lines": 2000 }
    },
    "review": {
      "enabled": true,
      "maxIterations": 3
    },
    "planning": {
      "maxPasses": 3
    },
    "work": {
      "maxAttemptsPerTask": 5,
      "maxWaves": 50,
      "stopOnBlock": false
    },
    "artifacts": {
      "enabled": true,
      "cleanupDays": 7
    },
    "memory": { "enabled": false },
    "planSync": { "enabled": false }
  }
}
```

| Section | Description |
|---------|-------------|
| `concurrency` | Parallel execution limits (workers only; planner always runs as a single agent) |
| `truncation` | Output size limits per agent role |
| `review` | Auto-review settings (note: `enabled` and `maxIterations` defined but not enforced) |
| `planning` | Planning loop settings (`maxPasses`, set to 1 for single-pass) |
| `work` | Execution limits (note: `maxWaves` and `maxAttemptsPerTask` defined but not enforced) |
| `artifacts` | Debug artifact storage |
| `memory` | Memory system (not yet implemented) |
| `planSync` | Auto-sync downstream specs (not yet implemented) |

## Task IDs

Simple format: `task-1`, `task-2`, `task-3`, ...

No epic prefixes needed.
