# @benvargas/pi-antigravity-image-gen

Image generation tool for [pi](https://github.com/badlogic/pi-mono) using Google Antigravity's Gemini 3 Pro Image model.

Generated images are returned as tool result attachments for inline terminal rendering.

Based on [opencode-antigravity-img](https://github.com/ominiverdi/opencode-antigravity-img) by ominiverdi and [opencode-antigravity-auth](https://github.com/NoeFabris/opencode-antigravity-auth) by NoeFabris.

## Features

- **Gemini 3 Pro Image** -- the only image model available via the Antigravity API
- **Aspect ratios** -- 1:1, 2:3, 3:2, 3:4, 4:3, 4:5, 5:4, 9:16, 16:9, 21:9
- **Inline rendering** -- images display directly in the terminal
- **Endpoint fallback** -- tries daily, autopush, and prod endpoints with automatic failover on rate limits or timeouts
- **Quota checking** -- check remaining image generation quota and reset time
- **Flexible save options** -- save to project, global, or custom directory
- **Config file support** -- persistent settings via JSON config

## Installation

```bash
pi install npm:@benvargas/pi-antigravity-image-gen
```

Or try without installing:

```bash
pi -e npm:@benvargas/pi-antigravity-image-gen
```

## Setup

Authenticate with Google Antigravity via OAuth:

```
/login
```

Select `google-antigravity` from the provider list. This stores OAuth credentials that the extension uses automatically.

## Usage

Once authenticated, ask pi to generate images naturally:

```
"Generate an image of a sunset over mountains"
"Create a 16:9 wallpaper of a cyberpunk city"
"Check my image generation quota"
```

The extension registers two tools that the model calls automatically:

### generate_image

Generate an image from a text prompt.

| Parameter | Description | Default |
|-----------|-------------|---------|
| `prompt` | Image description (required) | -- |
| `aspectRatio` | Image dimensions | `1:1` |
| `save` | Save mode (`none`, `project`, `global`, `custom`) | `none` |
| `saveDir` | Directory for `save=custom` | `PI_IMAGE_SAVE_DIR` |

### image_quota

Check remaining image generation quota. No parameters required.

Shows a progress bar with percentage remaining and time until quota resets. Image generation uses a separate quota from text models, resetting approximately every 5 hours.

## Saving Images

By default, images are only displayed inline and not saved to disk. Configure saving via tool parameter, environment variable, or config file (in order of priority):

### Tool parameter

The model can pass `save` directly when calling the tool.

### Environment variables

```bash
export PI_IMAGE_SAVE_MODE=global        # none|project|global|custom
export PI_IMAGE_SAVE_DIR=/path/to/dir   # required when mode=custom
```

### Config file

Create a JSON config file (project overrides global):

**Global**: `~/.pi/agent/extensions/antigravity-image-gen.json`
**Project**: `<repo>/.pi/extensions/antigravity-image-gen.json`

If neither exists, the extension writes a default config to the global path with `save` set to `global`.

```json
{
  "save": "global"
}
```

### Save locations

| Mode | Directory |
|------|-----------|
| `none` | Not saved |
| `project` | `<repo>/.pi/generated-images/` |
| `global` | `~/.pi/agent/generated-images/` |
| `custom` | `saveDir` param or `PI_IMAGE_SAVE_DIR` |

## API Endpoints

The extension uses Google's CloudCode API with automatic fallback:

1. `https://daily-cloudcode-pa.sandbox.googleapis.com` (primary)
2. `https://autopush-cloudcode-pa.sandbox.googleapis.com` (fallback)
3. `https://cloudcode-pa.googleapis.com` (production)

If an endpoint returns a 429 (rate limited) or times out, the next endpoint is tried automatically.

## Supported Models

Only **gemini-3-pro-image** is available via the Antigravity API. Other image models (Imagen, gemini-2.5-flash-image) are not supported by this endpoint.

Image output is always JPEG format. Generation typically takes 10-30 seconds.

## Requirements

- pi v0.51.0 or later
- Google Antigravity OAuth credentials (via `/login`)

## Uninstall

```bash
pi remove npm:@benvargas/pi-antigravity-image-gen
```

## License

MIT
