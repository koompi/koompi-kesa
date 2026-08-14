# Hooks

A hook is a shell command KESA runs at a fixed point in a turn.
It comes from your own settings file, runs with your privileges and is not sandboxed, exactly like a command you typed.

Hooks in a project's `.kesa/settings.json` stay inert until the global settings file sets `hooks.trustProjectHooks`, because a cloned repository can carry that file.

## The contract

Every hook reads a JSON object on stdin and answers with its exit code.

| Exit code | Meaning |
| --- | --- |
| `0` | Allow. |
| `2` | Block, and the hook's stderr becomes what the model sees. |
| anything else | Logged as a warning, then allowed. |

A hook that outlives its timeout is killed and treated as a warning.
The default timeout is 60 seconds, or 5 seconds for `SessionStart` and `SessionEnd`, which run where a stalled hook reads as a hung program.

## Events

| Event | When | Payload beyond `hook_event_name` and `cwd` |
| --- | --- | --- |
| `PreToolUse` | After the permission decision, before the tool spawns. Blocking stops the call. | `tool_name`, `tool_input` |
| `PostToolUse` | After the tool produced its result. Blocking appends feedback; it cannot undo the call. | `tool_name`, `tool_input`, `tool_response` |
| `UserPromptSubmit` | The user submitted a prompt. Blocking rejects it. | `prompt` |
| `Stop` | The agent finished a turn. Blocking asks for another. | `stop_hook_active` |
| `SessionStart` | The first turn of a session, before it reads input. | `session_id`, `source` (`startup` or `resume`) |
| `SessionEnd` | The session is over. See below. | `session_id`, `reason` (`exit`, `interrupt` or `panic`) |
| `PreCompact` | Before compaction discards anything. | `trigger` (`auto` or `manual`), `custom_instructions` |
| `SubagentStop` | A subagent finished a turn, in the subagent's own process. | `stop_hook_active` |
| `Notification` | KESA has something to tell you, including turn end. | `message` |

`matcher` filters on the tool name and only `PreToolUse` and `PostToolUse` carry one, so leave it off or set `*` for the rest.

```json
{
  "hooks": {
    "SessionEnd": [{ "command": "echo $(date) >> ~/.kesa/sessions.log", "timeout": 5 }],
    "Notification": [{ "command": "jq -r .message | xargs -0 notify-send KESA" }]
  }
}
```

## What reaches SessionEnd

Three paths fire it, whichever gets there first, and it fires once per session.

- The session guard's `Drop`, which covers a clean exit and any error that unwinds to one.
- A panic hook, chained onto whatever hook was already installed. It runs after the previous hook, so the panic message and the exit code are what they would have been anyway.
- A `SIGINT` handler, installed on the first turn and only when nothing else in the process has claimed `SIGINT`. The RPC, ACP and print modes claim it for their own graceful shutdown, and that shutdown unwinds to the `Drop` above.

### What it misses

- `SIGKILL` and `SIGSTOP`. Neither can be caught by any process, so no hook can run. This is not a gap that can be closed.
- `SIGTERM`, `SIGHUP` and `SIGQUIT`. The interrupt handler is `ctrlc` without its `termination` feature, so it covers `SIGINT` alone.
- A `SIGINT` that arrives before the first turn, since the handler is installed there rather than at startup.
- `std::process::exit`, `abort`, and a build with `panic = "abort"`. None of them unwind, so no destructor runs.
- The process being killed by the OOM killer, and power loss.

## Turn-end notification

When a turn ends, KESA writes an OSC 9 desktop notification to the terminal, or a bell where the terminal does not understand OSC 9.
This bypasses the renderer: both are terminal-level sequences and neither belongs in a frame.

It fires only when the terminal is unfocused.
A notification that arrives while you are watching the screen is noise, and noise gets muted, after which the feature is worse than absent.

Focus comes from DEC mode 1004 focus reporting, which only the code holding the terminal's input can see.
Until something calls `notify::set_terminal_focus`, focus is unknown and the only condition on notifying is `KESA_NOTIFY`.

| Variable | Effect |
| --- | --- |
| `KESA_NOTIFY=0` | No notification, and no `Notification` hook. |
| `KESA_NOTIFY_OSC=0` | Force the bell instead of OSC 9. |
| `KESA_NOTIFY_OSC=1` | Force OSC 9 in a terminal that was not recognised. |

OSC 9 is used in kitty, WezTerm, ghostty, iTerm2, Konsole, Windows Terminal, foot, Hyper and rio.
Inside tmux or screen it falls back to the bell, which those already turn into a window activity flag.

The `Notification` hook runs on the same decision, so a hook that calls `notify-send` is silenced by focus exactly as the bell is, and KESA never grows a desktop dependency of its own.
