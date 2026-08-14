#!/bin/bash
# TDD-08-1: /shadow-git rollback command should exist
set -e
EXT="${EXT:-$HOME/.pi/agent/extensions/shadow-git.ts}"

# Check if source has rollback handler
if grep -q "rollback" "$EXT" && grep -q "registerCommand\|case.*rollback" "$EXT"; then
  echo "PASS: rollback command exists"
  exit 0
else
  echo "FAIL: rollback command not found"
  exit 1
fi
