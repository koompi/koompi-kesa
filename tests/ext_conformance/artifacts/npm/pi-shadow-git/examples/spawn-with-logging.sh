#!/bin/bash
#
# Example: Spawn a pi agent with shadow-git logging
#
# Usage:
#   ./spawn-with-logging.sh <workspace-dir> <agent-name> "<prompt>"
#
# Example:
#   ./spawn-with-logging.sh ~/workspaces/task-001 scout1 "Research X and write findings"

set -e

WORKSPACE_DIR="${1:?Usage: $0 <workspace-dir> <agent-name> \"<prompt>\"}"
AGENT_NAME="${2:?Usage: $0 <workspace-dir> <agent-name> \"<prompt>\"}"
PROMPT="${3:?Usage: $0 <workspace-dir> <agent-name> \"<prompt>\"}"

# Resolve paths
if [ ! -d "$WORKSPACE_DIR" ]; then
    mkdir -p "$WORKSPACE_DIR"
fi
WORKSPACE_DIR="$(cd "$WORKSPACE_DIR" && pwd)"

EXTENSION_PATH="$(dirname "$0")/../src/shadow-git.ts"
EXTENSION_PATH="$(cd "$(dirname "$EXTENSION_PATH")" && pwd)/$(basename "$EXTENSION_PATH")"

# Initialize workspace if needed
if [ ! -d "$WORKSPACE_DIR/.git" ]; then
    echo "Initializing git in $WORKSPACE_DIR"
    git -C "$WORKSPACE_DIR" init
    git -C "$WORKSPACE_DIR" commit --allow-empty -m "Initial workspace"
fi

# Create agent directory
AGENT_DIR="$WORKSPACE_DIR/agents/$AGENT_NAME"
mkdir -p "$AGENT_DIR"/{workspace,output}

echo "Workspace: $WORKSPACE_DIR"
echo "Agent: $AGENT_NAME"
echo "Extension: $EXTENSION_PATH"
echo ""

# Write spawn script to temp file to avoid shell quoting issues
# This ensures all arguments are passed correctly to pi
SPAWN_SCRIPT=$(mktemp)
cat > "$SPAWN_SCRIPT" << SPAWN_EOF
#!/bin/bash
cd "$AGENT_DIR"
export PI_WORKSPACE_ROOT="$WORKSPACE_DIR"
export PI_AGENT_NAME="$AGENT_NAME"

pi \\
  --model claude-haiku-4-5 \\
  --tools read,write,bash \\
  --max-turns 30 \\
  --no-input \\
  -e "$EXTENSION_PATH" \\
  "$PROMPT" \\
  2>&1 | tee output/run.log

echo ""
echo "Agent completed. Press enter to close."
read
SPAWN_EOF

chmod +x "$SPAWN_SCRIPT"

# Spawn in tmux using the temp script
tmux new-session -d -s "$AGENT_NAME" "bash '$SPAWN_SCRIPT'"

echo "Spawned agent '$AGENT_NAME' in tmux session"
echo ""
echo "Commands:"
echo "  tmux attach -t $AGENT_NAME      # Observe agent"
echo "  git -C $WORKSPACE_DIR log       # View commits"
echo "  cat $AGENT_DIR/audit.jsonl      # View events"
echo ""
echo "Killswitch (if needed):"
echo "  In agent: /shadow-git disable"
echo "  Or: PI_SHADOW_GIT_DISABLED=1 when spawning"
