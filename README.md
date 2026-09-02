<h1 align="center">KOOMPI KESA</h1>

<p align="center">
  <strong>A coding agent that lives in your terminal, written in Rust, built for KOOMPI OS.</strong>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/rust-2024%20edition-orange?logo=rust" alt="Rust 2024">
  <img src="https://img.shields.io/badge/license-MIT%20%2B%20Rider-blue" alt="License: MIT + Rider">
  <img src="https://img.shields.io/badge/unsafe-forbidden-brightgreen" alt="No unsafe code">
  <img src="https://img.shields.io/badge/sandbox-landlock-blue" alt="Landlock sandbox">
</p>

```bash
curl -fsSL https://raw.githubusercontent.com/koompi/koompi-kesa/main/install.sh | bash
```

Then run `kesa` in any project directory.

---

## What it is

`kesa` is a terminal coding agent: you describe what you want, it reads your
files, edits them, runs commands, and shows its work as it goes. One static
binary, no runtime to install, no Electron.

It is a hard fork of [pi_agent_rust](https://github.com/Dicklesworthstone/pi_agent_rust)
maintained for KOOMPI OS. See [NOTICE.md](NOTICE.md) for attribution and the
licence rider, which travels with every copy.

## Permissions

`kesa` never runs a tool the policy has not cleared. The mode sets the
baseline; shift+tab cycles it mid-session.

| Mode | Reads | Edits | Shell |
|---|---|---|---|
| `default` | runs | asks | asks |
| `accept-edits` | runs | runs | asks |
| `plan` | runs | refuses | refuses |
| `read-only` | runs | refuses | refuses |

Rules override the mode and are matched deny-first, so a deny always wins:

```json
{
  "permissions": {
    "mode": "default",
    "deny": ["Bash(rm:*)", "Bash(curl:*)"],
    "allow": ["Bash(git status:*)", "Bash(cargo test:*)"]
  }
}
```

Per run: `--permission-mode`, `--deny-tool`, `--allow-tool`. CLI beats env
beats project settings beats global, and CLI rules are added to the configured
ones rather than replacing them, so a flag can only narrow what is permitted.

When a call needs a decision, the answer box appears in the transcript and the
turn waits for it. Choosing "don't ask again" scopes a shell grant to the first
word of the command for the rest of the session: approving `git status` does
not also approve `rm`.

## Sandbox

Every command the shell tool starts is confined with
[landlock](https://landlock.io) on Linux. It may read the system directories a
shell needs to run, read and write your workspace, and nothing else. Your SSH
keys, your browser profile and your home directory are unreachable from inside
a command the model wrote.

`--sandbox-write <dir>` grants an extra writable directory. `--no-sandbox`
turns it off, and is required rather than assumed: on a kernel without
landlock, `kesa` refuses to run commands instead of quietly running them
unconfined.

Landlock is Linux-only. On other platforms the sandbox reports itself
unavailable and the same refusal applies.

## Install

```bash
# Latest release
curl -fsSL https://raw.githubusercontent.com/koompi/koompi-kesa/main/install.sh | bash

# A specific version, or a different directory
curl -fsSL https://raw.githubusercontent.com/koompi/koompi-kesa/main/install.sh \
  | bash -s -- --version v0.4.0 --dest /usr/local/bin
```

The installer checks the download against the release's `SHA256SUMS` before
installing anything. `KESA_DOWNLOAD_BASE` points it at a mirror.

From source, with the pinned nightly in `rust-toolchain.toml`:

```bash
git clone https://github.com/koompi/koompi-kesa.git
cd koompi-kesa
cargo install --path .
```

Uninstall:

```bash
curl -fsSL https://raw.githubusercontent.com/koompi/koompi-kesa/main/uninstall.sh | bash
```

## Use

```bash
kesa                                  # interactive session in the current directory
kesa "explain the retry logic in src/http"
kesa -p "what does this error mean?" < error.log   # print mode, no session
kesa --continue                       # pick up the last session
kesa --resume                         # choose a session from the picker
kesa --permission-mode plan "how would you restructure the parser?"
```

Point it at a provider with `--provider` and `--model`, or set `KESA_PROVIDER`
and `KESA_MODEL`. Local models work the same way: Ollama and LM Studio get a
ten-minute first-request timeout so a cold model load does not read as a
hang.

Tools: `read`, `write`, `edit`, `hashline_edit`, `bash`, `grep`, `find`, `ls`,
`subagent`.

## Configure

Settings live in `~/.kesa/agent/settings.json`, and a project can override them
from `.kesa/settings.json` in the repository. Project settings merge over global
ones rather than replacing them.

Skills, prompts, themes and extensions load from `~/.kesa/agent/` and from
`.kesa/` in the project. `kesa config` opens the settings UI, `kesa doctor`
reports on the environment, and `kesa list` shows what is installed.

## Documentation

[docs/README.md](docs/README.md) is the index. The ones most people want are
[settings](docs/settings.md), [tui](docs/tui.md), [providers](docs/providers.md)
and [troubleshooting](docs/troubleshooting.md).

## Building

```bash
cargo check --all-targets
cargo test --lib
cargo build --release --bin kesa
```

`unsafe_code` is forbidden crate-wide. Anything needing privileged setup goes
through a re-exec rather than a relaxed lint.

## Licence

MIT, plus a rider that withholds every right from OpenAI, Anthropic and their
affiliates. The rider covers derivative works and must ship unmodified. See
[LICENSE](LICENSE) and [NOTICE.md](NOTICE.md).
