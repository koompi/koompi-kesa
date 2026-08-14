# Skills Template

This file documents the skills structure and how to add new skills.

## Skill Structure

A skill is a directory with a `SKILL.md` file. Everything else is freeform.

```
my-skill/
├── SKILL.md              # Required: frontmatter + instructions
├── scripts/              # Helper scripts (optional)
│   └── process.sh
├── references/           # Detailed docs loaded on-demand (optional)
│   └── api-reference.md
└── assets/               # Static assets (optional)
    └── template.json
```

## SKILL.md Format

```markdown
---
name: my-skill
description: What this skill does and when to use it. Be specific.
license: MIT
compatibility: Node.js 20+, Python 3.11+
---

# My Skill

## Setup

Run once before first use:
\`\`\`bash
cd /path/to/skill && npm install
\`\`\`

## Usage

\`\`\`bash
./scripts/process.sh <input>
\`\`\`

## References

See [the reference guide](references/REFERENCE.md) for details.
```

## Frontmatter

Per the [Agent Skills specification](https://agentskills.io/specification#frontmatter-required):

| Field | Required | Description |
|-------|----------|-------------|
| `name` | Yes | Max 64 chars. Lowercase a-z, 0-9, hyphens. Must match parent directory. |
| `description` | Yes | Max 1024 chars. What the skill does and when to use it. |
| `license` | No | License name or reference to bundled file. |
| `compatibility` | No | Max 500 chars. Environment requirements. |

## Name Rules

- 1-64 characters
- Lowercase letters, numbers, hyphens only
- No leading/trailing hyphens
- No consecutive hyphens
- Must match parent directory name

Valid: `pdf-processing`, `data-analysis`, `code-review`
Invalid: `PDF-Processing`, `-pdf`, `pdf--processing`

## Description Best Practices

The description determines when the agent loads the skill. Be specific.

Good:
```yaml
description: Extracts text and tables from PDF files, fills PDF forms, and merges multiple PDFs. Use when working with PDF documents.
```

Poor:
```yaml
description: Helps with PDFs.
```

## Skill Discovery

Pi automatically discovers skills from:
- Global: `~/.pi/agent/skills/`
- Project: `.pi/skills/`
- Packages: `skills/` directories in pi packages

Use relative paths in your SKILL.md to reference scripts and assets:

```markdown
See [the reference guide](references/REFERENCE.md) for details.

Run the script:
\`\`\`bash
./scripts/process.sh
\`\`\`
```
