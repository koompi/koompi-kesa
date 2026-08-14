# Pi Canvas

Interactive TUI canvases for [Pi Coding Agent](https://github.com/badlogic/pi-mono). Display calendars, documents, and flight search results directly in pi's terminal UI.

**No external dependencies required** - works anywhere pi works!

## Features

- 📅 **Calendar Canvas** - Week view calendar with event display and time slot selection
- 📄 **Document Canvas** - Text viewer with line navigation and selection
- ✈️ **Flight Canvas** - Flight search results with comparison and selection

## Installation

```bash
# Clone to extensions directory
git clone https://github.com/jyaunches/pi-canvas ~/.pi/agent/extensions/pi-canvas

# Restart pi to load the extension
```

## Important: Data Sources

**Pi Canvas is a display/UI layer only** - it does not fetch data from external services. The LLM provides the data to display.

### How It Works

```
┌─────────────────────────────────────────────────────────────────┐
│                        Typical Flow                              │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  1. User: "Schedule a meeting with Alice next week"              │
│                                                                  │
│  2. LLM fetches data using OTHER tools/skills:                   │
│     - Calls Google Calendar skill to get Alice's busy times      │
│     - Calls Google Calendar skill to get your busy times         │
│                                                                  │
│  3. LLM formats data as JSON and calls canvas_calendar           │
│                                                                  │
│  4. User sees interactive calendar, navigates, selects time      │
│                                                                  │
│  5. LLM receives selection, takes action (e.g., creates event)   │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### Recommended Data Source Skills

To make canvases useful with real data, combine with these [pi-skills](https://github.com/badlogic/pi-skills):

| Canvas | Recommended Skill | Description |
|--------|-------------------|-------------|
| Calendar | [gccli](https://github.com/badlogic/pi-skills/tree/main/gccli) | Google Calendar CLI |
| Document | [gmcli](https://github.com/badlogic/pi-skills/tree/main/gmcli) | Gmail CLI for emails |
| Document | Local files | Pi's built-in `read` tool |
| Flights | External API | Amadeus, Skyscanner, etc. (not included) |

### Example: Calendar with Google Calendar

```bash
# 1. Install gccli skill (see pi-skills repo for setup)

# 2. Ask pi:
"Show me my calendar for this week"

# LLM will:
# - Use gccli to fetch your events
# - Call canvas_calendar with the event data
# - Display interactive calendar
```

## Tools

### canvas_calendar

Display an interactive calendar TUI where users can navigate and select time slots.

```
Parameters:
- title: Calendar title (optional)
- events: Array of calendar events (optional)
- weekOffset: Week offset, 0=current, -1=last, 1=next (optional)
```

**Event format:**
```json
{
  "id": "1",
  "title": "Team Meeting",
  "startTime": "2024-01-15T10:00:00",
  "endTime": "2024-01-15T11:00:00",
  "color": "blue"
}
```

**Colors:** blue, green, yellow, red, magenta, cyan

**Keyboard:**
- `←→↑↓` or `hjkl` - Navigate days/hours
- `n/p` - Next/previous week
- `t` - Jump to today
- `Enter` - Select time slot
- `Esc` or `q` - Cancel

### canvas_document

Display a document in an interactive TUI viewer with navigation and text selection.

```
Parameters:
- content: Document content to display (required)
- title: Document title (optional)
```

**Keyboard:**
- `↑↓` or `jk` - Navigate lines
- `Ctrl+U/D` - Page up/down
- `v` - Start/extend selection
- `Enter` - Confirm selection
- `Esc` or `q` - Close

### canvas_flights

Display flight search results in an interactive TUI for comparison and selection.

```
Parameters:
- title: Title for the search results (optional)
- flights: Array of flight objects (required)
```

**Flight format:**
```json
{
  "id": "1",
  "airline": "United Airlines",
  "flightNumber": "UA 123",
  "origin": { "code": "SFO", "name": "San Francisco International", "city": "San Francisco" },
  "destination": { "code": "JFK", "name": "John F. Kennedy International", "city": "New York" },
  "departureTime": "2024-01-15T08:00:00",
  "arrivalTime": "2024-01-15T16:30:00",
  "duration": 330,
  "price": 35000,
  "currency": "USD",
  "stops": 0,
  "aircraft": "Boeing 737-800"
}
```

**Keyboard:**
- `↑↓` or `jk` - Navigate flights
- `Enter` - Select flight
- `Esc` or `q` - Cancel

## Commands

### /calendar

Open a standalone calendar view:

```
/calendar
```

## Architecture

Pi Canvas renders directly in pi's TUI using `ctx.ui.custom()`. This means:

- ✅ No tmux required
- ✅ No Bun runtime required  
- ✅ Works in any terminal
- ✅ Uses pi's native theming
- ✅ Seamless integration

```
┌─────────────────────────────────────────────────────────┐
│                      Pi Terminal                         │
├─────────────────────────────────────────────────────────┤
│                                                          │
│   ┌─────────────────────────────────────────────────┐   │
│   │              Canvas Component                    │   │
│   │   (calendar, document, or flights)               │   │
│   │                                                  │   │
│   │   User navigates with keyboard                   │   │
│   │   ESC to close, Enter to select                  │   │
│   └─────────────────────────────────────────────────┘   │
│                                                          │
│   LLM receives result directly                           │
│                                                          │
└─────────────────────────────────────────────────────────┘
```

### Comparison with Original claude-canvas

This extension is inspired by [claude-canvas](https://github.com/dvdsgl/claude-canvas) but rewritten for pi:

| Feature | claude-canvas | pi-canvas |
|---------|---------------|-----------|
| Runtime | Requires tmux + Bun | None (native pi) |
| Rendering | Separate process + IPC | Inline via `ctx.ui.custom()` |
| Components | React/Ink | Native pi TUI |
| Theming | Custom | Uses pi's theme |
| Data sources | None (LLM provides) | None (LLM provides) |

## Example Prompts

Try these with pi (assuming you have relevant data source skills installed):

**Calendar:**
- "Show me my calendar for this week"
- "Find a time when Alice and Bob are both free"
- "What meetings do I have tomorrow?"

**Document:**
- "Let me review this file" (then select portions)
- "Show me the README and let me pick a section to edit"

**Flights:**
- "Search for flights from SFO to JFK" (requires flight API integration)
- "Compare these flight options" (with mock/provided data)

## Development

The extension is a single TypeScript file (`index.ts`) that registers:
- 3 tools (`canvas_calendar`, `canvas_document`, `canvas_flights`)
- 1 command (`/calendar`)
- Session startup handler

To modify, edit `~/.pi/agent/extensions/pi-canvas/index.ts` and restart pi.

## Credits

Inspired by [claude-canvas](https://github.com/dvdsgl/claude-canvas) by David Siegel.

## License

MIT
