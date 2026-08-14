# Pi OpenRouter Extension

[![npm version](https://badge.fury.io/js/%40verioussmith%2Fpi-openrouter.svg)](https://www.npmjs.com/package/@verioussmith/pi-openrouter)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

OpenRouter provider extension for [Pi Coding Agent](https://github.com/badlogic/pi-mono).

## Features

- 🚀 Access 20+ LLM models through OpenRouter's unified API
- 🔄 OpenAI-compatible streaming
- 🤖 Supports Claude, GPT-4, Gemini, Llama, Mistral, DeepSeek, Perplexity
- 🆓 Free models available (Gemini 2.0 Flash)
- 📦 One-command installation via npm
- ⚡ Auto-installs to `~/.pi/agent/extensions/`

## Installation

### Quick Install (Recommended)

```bash
npm install -g @verioussmith/pi-openrouter
```

That's it! The extension automatically installs to `~/.pi/agent/extensions/openrouter.ts`

### Alternative Methods

See [INSTALL.md](INSTALL.md) for manual installation options.

## Configuration

Set your OpenRouter API key in `~/.zshrc` or environment:

```bash
export OPENROUTER_API_KEY="sk-or-v1-YOUR_KEY_HERE"
```

Get your key at: https://openrouter.ai/settings/keys

## Usage

```bash
# Start pi
pi

# List available models
/model

# Select an OpenRouter model
/model openrouter/google/gemini-2.0-flash-exp:free

# Or any other model
/model openrouter/anthropic/claude-3.5-sonnet
/model openrouter/openai/gpt-4o
/model openrouter/deepseek/deepseek-r1
```

## Available Models

### Free Tier
- `google/gemini-2.0-flash-exp:free` - Gemini 2.0 Flash Experimental
- `google/gemini-2.0-flash-thinking-exp:free` - Gemini 2.0 Flash with reasoning

### Anthropic
- `anthropic/claude-3.5-sonnet` - Claude 3.5 Sonnet
- `anthropic/claude-3-opus` - Claude 3 Opus
- `anthropic/claude-3-haiku` - Claude 3 Haiku

### OpenAI
- `openai/gpt-4o` - GPT-4o
- `openai/gpt-4o-mini` - GPT-4o Mini
- `openai/o1` - O1 (reasoning model)
- `openai/o1-mini` - O1 Mini

### Google
- `google/gemini-pro-1.5` - Gemini Pro 1.5

### Meta
- `meta-llama/llama-3.3-70b-instruct` - Llama 3.3 70B
- `meta-llama/llama-3.1-405b-instruct` - Llama 3.1 405B

### Mistral
- `mistralai/mistral-large` - Mistral Large
- `mistralai/mistral-small` - Mistral Small

### DeepSeek
- `deepseek/deepseek-chat` - DeepSeek Chat
- `deepseek/deepseek-r1` - DeepSeek R1 (reasoning)

### Perplexity
- `perplexity/sonar-pro` - Sonar Pro

See all models: https://openrouter.ai/models

## Adding More Models

Edit `openrouter.ts` and add to the `models` array:

```typescript
{
  id: "provider/model-name",
  name: "Display Name (OpenRouter)",
  reasoning: false, // true for reasoning models
  input: ["text", "image"], // or just ["text"]
  cost: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0 },
  contextWindow: 128000,
  maxTokens: 4096,
}
```

Costs are in $/million tokens. Check https://openrouter.ai/models for pricing.

## Development

```bash
# Clone
git clone https://github.com/verioussmith/pi-openrouter-extension.git
cd pi-openrouter-extension

# Install in Pi (development mode)
ln -s $(pwd)/openrouter.ts ~/.pi/agent/extensions/openrouter.ts

# Test
pi
/model openrouter/google/gemini-2.0-flash-exp:free
```

### Publishing Updates

```bash
# Update version in package.json
npm version patch  # or minor, major

# Commit and push
git add . && git commit -m "chore: bump version" && git push

# Publish to npm (requires OTP)
npm publish --access public --otp=YOUR_OTP
```

## Package Links

- **npm:** https://www.npmjs.com/package/@verioussmith/pi-openrouter
- **GitHub:** https://github.com/verioussmith/pi-openrouter-extension
- **Issues:** https://github.com/verioussmith/pi-openrouter-extension/issues

## References

- [Pi Coding Agent](https://github.com/badlogic/pi-mono)
- [Pi Custom Provider Docs](https://github.com/badlogic/pi-mono/blob/main/packages/coding-agent/docs/custom-provider.md)
- [OpenRouter API](https://openrouter.ai/docs)
- [OpenRouter Models](https://openrouter.ai/models)

## Contributing

Contributions welcome! Please:
1. Fork the repo
2. Create a feature branch
3. Submit a pull request

## Author

**Verious Smith** ([@verioussmith](https://github.com/verioussmith))
- Website: https://philoveracity.com
- Email: verious@philoveracity.com

## License

MIT - see [LICENSE](LICENSE) file for details
