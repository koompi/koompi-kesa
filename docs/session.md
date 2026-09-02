# Sessions

KESA stores conversation history in session files.

## Current Storage Model (V1)

### File format

Sessions are stored as JSONL (JSON Lines) files.

### Location

Sessions are grouped by project directory:
`~/.kesa/agent/sessions/--encoded-project-path--/`

Filename format: `YYYY-MM-DDTHH-MM-SS.sssZ_id.jsonl`

### Structure

1. Header: the first line is always a `SessionHeader` object containing metadata (ID, timestamp, CWD, initial settings).
2. Entries: subsequent lines are `SessionEntry` objects representing events in the conversation.

### Entry types

- `message`: User or Assistant message.
- `model_change`: User switched models.
- `thinking_level_change`: User changed thinking settings.
- `compaction`: Context was summarized to save tokens.
- `branch_summary`: A summary of a branch point (when forking).
- `session_info`: Updates like session renaming.
- `label`: Metadata label assignment on an entry.
- `custom`: Extension-defined structured payload.

### Tree structure

KESA supports conversation branching. Each entry has an `id` and an optional `parent_id`.

- Linear conversation: `A -> B -> C`
- Branching:
  ```
  A -> B -> C
       \ -> D
  ```

When you navigate to a previous message and reply, KESA creates a new branch.

### Management

#### Resume (`/resume`, `pi -r`)

Opens the session picker to switch between sessions.
- Select: Enter
- Delete: Ctrl+D (requires confirmation)

#### Tree navigator (`/tree`)

Visualizes the branching structure of the current session.
- Navigate: Up/Down
- Switch: Enter (switches active context to the selected node)

#### Forking (`/fork`)

Creates a new session file starting from the current point (or a selected point). This is useful when you want to explore a significantly different direction without cluttering the current session file.

#### Compaction (`/compact`)

Manually triggers context compaction. KESA also compacts automatically based on the `compaction` settings in `settings.json`.
