# Prompt Templates Template

This file documents how to add prompt templates to this package.

## Template Structure

Prompt templates are markdown files with reusable prompt patterns.

```
prompts/
├── code-review.md        # Code review template
├── refactor.md          # Refactoring template
└── README.md             # Template descriptions
```

## Basic Template

```markdown
---
name: code-review
description: Review code for bugs, security issues, and best practices
---

# Code Review

Please review the following code for:

- **Bugs**: Logic errors, edge cases, null/undefined handling
- **Security**: Input validation, SQL injection, XSS, authentication
- **Performance**: Inefficient algorithms, unnecessary computations
- **Readability**: Naming, structure, comments, documentation
- **Best Practices**: Language conventions, design patterns

Provide specific, actionable feedback with code examples where applicable.

## Focus Areas

{{focus_areas}}

## Files to Review

{{files}}
```

## Using Templates

After installing the package, use templates via:

```
/prompt:code-review
/prompt:code-review files:src/main.py focus_areas:security,performance
```

## Template Variables

Templates can use `{{variable}}` syntax for placeholders. Pass values as arguments:

```
/prompt:template-name key:value another_key:value2
```

## Template Discovery

Templates are auto-discovered from:
- Global: `~/.pi/agent/prompts/`
- Project: `.pi/prompts/`
- Packages: `prompts/` directories
