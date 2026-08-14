#!/bin/bash
# TDD-09-1: /shadow-git branch command should exist
set -e
EXT="${EXT:-$HOME/.pi/agent/extensions/shadow-git.ts}"

# Check if source has branch handler
if grep -q "case.*branch" "$EXT" || grep -q '"branch"' "$EXT"; then
  echo "PASS: branch command exists"
  exit 0
else
  echo "FAIL: branch command not found"
  exit 1
fi
