# pi-moonshot

Pi extension adding [Moonshot AI](https://www.moonshot.ai/) provider support, including the Kimi K2.5 model family.

## Installation

```bash
pi install npm:pi-moonshot
```

Or try without installing:

```bash
pi -e npm:pi-moonshot
```

## Setup

Set your Moonshot API key:

```bash
export MOONSHOT_API_KEY="your-api-key"
```

Or add to your `~/.pi/agent/settings.json`:

```json
{
  "apiKeys": {
    "MOONSHOT_API_KEY": "your-api-key"
  }
}
```

## Models

| Model | Description | Context | Reasoning |
|-------|-------------|---------|-----------|
| `kimi-k2.5` | Flagship model with vision | 262K | Yes |
| `kimi-latest` | Latest stable version | 131K | No |
| `kimi-k2-turbo-preview` | Fast inference model | 262K | No |
| `kimi-k2-thinking` | Reasoning model | 262K | Yes |
| `moonshot-v1-8k` | Legacy 8K context | 8K | No |
| `moonshot-v1-32k` | Legacy 32K context | 32K | No |
| `moonshot-v1-128k` | Legacy 128K context | 128K | No |

## Usage

After installation, select a Moonshot model:

```bash
pi /model moonshot/kimi-k2.5
```

Or start pi directly with a Moonshot model:

```bash
pi --provider moonshot --model kimi-k2.5
```

## License

MIT