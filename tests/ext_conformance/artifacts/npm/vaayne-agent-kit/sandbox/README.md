# Agent Sandbox

A containerized development environment for running AI coding agents in isolation.

## Features

- **Debian 13** base image
- **Non-root user** (`dev`, uid 10001) for security
- Pre-installed tools via [mise](https://mise.jdx.dev/):
  - Node.js 23
  - Bun (latest)
  - uv (latest)
  - ripgrep (`rg`)
  - fd
- [Pi Coding Agent](https://github.com/mariozechner/pi-coding-agent) pre-installed

## Quick Start

```bash
# Pull the image
docker pull vaayne/agent-sandbox:latest

# Run interactively
docker run -it --rm vaayne/agent-sandbox:latest bash

# Run with a mounted workspace
docker run -it --rm -v $(pwd):/home/dev/workspace vaayne/agent-sandbox:latest bash
```

## Usage with Pi Agent

```bash
# Run pi agent in the container
docker run --rm -it \
    --network host \
    -v "$(pwd):/home/dev/workspace" \
    -v "$HOME/.pi:/home/dev/.pi" \
    vaayne/agent-sandbox:latest
```

## Building

### Build locally (current architecture)

```bash
cd sandbox
docker build -t vaayne/agent-sandbox -f Dockerfile .
```

### Build multi-arch and push

```bash
# Using mise task (recommended)
mise run sandbox:build

# Or manually
docker buildx build --platform linux/amd64,linux/arm64 \
  -t vaayne/agent-sandbox:latest \
  -f Dockerfile \
  --push .
```

## Build Arguments

Customize versions during build:

| Argument       | Default  | Description                |
| -------------- | -------- | -------------------------- |
| `NODE_VERSION` | `23`     | Node.js version            |
| `UV_VERSION`   | `latest` | uv package manager version |
| `BUN_VERSION`  | `latest` | Bun runtime version        |
| `PI_VERSION`   | `latest` | Pi coding agent version    |

Example:

```bash
docker build \
  --build-arg NODE_VERSION=22 \
  --build-arg PI_VERSION=0.49.0 \
  -t vaayne/agent-sandbox:custom .
```

## Architectures

Multi-arch support for:

- `linux/amd64`
- `linux/arm64`

## Environment

| Variable  | Value                 |
| --------- | --------------------- |
| `HOME`    | `/home/dev`           |
| `WORKDIR` | `/home/dev/workspace` |
| User      | `dev` (uid 10001)     |

## License

MIT
