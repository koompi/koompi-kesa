#!/bin/bash
# TDD-10-1: No execSync for git commands (should be async)
set -e
EXT="${EXT:-$HOME/.pi/agent/extensions/shadow-git.ts}"

# Check for execSync usage with git
if grep -q 'execSync.*git\|execSync("git' "$EXT"; then
  echo "FAIL: execSync used for git commands"
  grep -n "execSync" "$EXT" | head -3
  exit 1
else
  echo "PASS: no execSync for git"
  exit 0
fi
