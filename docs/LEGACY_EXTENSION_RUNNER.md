## Legacy pi‑mono Extension Runner (Pinned)

This document pins the **exact legacy environment** used for extension reference captures and lists the **commands** to reproduce runs.

---

## 1) Repo Pin

**Source repo:** `https://github.com/badlogic/pi-mono`
**Local vendored snapshot:** `legacy_pi_mono_code/pi-mono/`
**Commit:** `df5b0f76c026b35fdd7f0fb78cb0dbaaf939c1b5`

Verify:
```bash
cat legacy_pi_mono_code/pi-mono/PINNED_COMMIT
test -f legacy_pi_mono_code/pi-mono/package-lock.json
test -x legacy_pi_mono_code/pi-mono/pi-test.sh
test -f legacy_pi_mono_code/pi-mono/packages/coding-agent/src/cli.ts
```

The snapshot is vendored as source files, not as a nested Git checkout, so
`git -C legacy_pi_mono_code/pi-mono rev-parse HEAD` resolves the parent Rust
repository and is not a valid pin check.

---

## 2) Runtime & Dependency Pinning

**Node engine requirement:** `>=20.0.0` (from `package.json`).

**Dependency lock:** `package-lock.json` (use `npm ci` for reproducibility).

**Workspace packages:** `packages/*` plus extension examples in  
`packages/coding-agent/examples/extensions/*`.

Install/build (from repo root):
```bash
cd legacy_pi_mono_code/pi-mono
npm ci
npm run build
```

> `npm run check` requires `npm run build` first.

---

## 3) Running the Legacy CLI (from sources)

Convenience wrapper:
```bash
./pi-test.sh
```

This executes:
```bash
npx tsx packages/coding-agent/src/cli.ts
```

**No‑env mode** (clear API keys for deterministic tests):
```bash
./pi-test.sh --no-env
```

---

## 4) Extension Loading (Examples + Local)

**Load a single extension via CLI:**
```bash
./pi-test.sh --extension packages/coding-agent/examples/extensions/permission-gate.ts
```

**Auto‑discover by copying into extensions dir:**
```bash
cp packages/coding-agent/examples/extensions/permission-gate.ts ~/.kesa/agent/extensions/
./pi-test.sh
```

**Repo‑local extensions** (already present):
```
legacy_pi_mono_code/pi-mono/.kesa/extensions/
```

---

## 5) Pi Packages (npm or git)

Install packages with extensions/skills/prompts/themes:
```bash
./pi-test.sh install npm:@foo/pi-tools
./pi-test.sh install npm:@foo/pi-tools@1.2.3
./pi-test.sh install git:github.com/user/repo
./pi-test.sh install git:github.com/user/repo@v1
```

Packages install to:
```
~/.kesa/agent/git/   (git)
~/.kesa/agent/npm/   (npm)
```

For project‑local installs:
```bash
./pi-test.sh install -l npm:@foo/pi-tools
```

---

## 6) Capture Baseline Workflow (Suggested)

1. **Ensure pin:** checkout the exact commit and run `npm ci`.  
2. **Select extension:** from `examples/extensions/` or `.kesa/extensions/`.  
3. **Run with deterministic env:** prefer `--no-env` unless API keys are required.  
4. **Record outputs:** capture stdout/stderr and any session JSONL outputs.

Example capture command:
```bash
./pi-test.sh --no-env --extension packages/coding-agent/examples/extensions/permission-gate.ts
```

---

## 7) Notes

- The example extensions list and descriptions live at:  
  `packages/coding-agent/examples/extensions/README.md`
- For extension docs, see:  
  `packages/coding-agent/docs/extensions.md`
