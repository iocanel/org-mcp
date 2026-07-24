# org-cli

A CLI and MCP (Model Context Protocol) server for org-mode and org-roam integration.

Run as an MCP server with `org-cli --mcp`, or use the subcommands directly (`org-cli agenda today`, `org-cli habits ...`, `org-cli drill ...`).

## Features

### Agenda Tools
- `get_agenda_today` - Get today's agenda including tasks, habits, and events
- `get_agenda_upcoming` - Get upcoming agenda items for the next N days

### Inbox Management
- `query_inbox` - Query inbox items, optionally filtering by section (personal, work, email)
- `add_to_inbox` - Add new items to the inbox

### Habit Tracking
- `get_habits` - Get all habits with their current status
- `get_habits_due_today` - Get habits due today
- `create_habit` - Create a new recurring habit (TODO with a repeating SCHEDULED and :STYLE: habit)
- `mark_habit_done` - Mark a habit as done for today (org-habit reschedules it)
- `delete_habit` - Delete a habit by ID or title

### org-drill
- `drill_status` - Show drill card counts (total, due-scheduled, new-unscheduled)
- `drill_start` - Launch an interactive org-drill session in a new Emacs frame (non-blocking)

### Task Management
- `create_task` - Create a new task in a specified file
- `complete_task` - Mark a task as complete
- `update_task_scheduled` - Update the scheduled date of a task
- `update_task_deadline` - Update the deadline of a task
- `refile_task` - Refile a task to a different file or heading

### Org-roam Integration
- `search_nodes` - Search for nodes by title, tags, or aliases
- `get_node` - Get detailed information about a specific node
- `get_backlinks` - Get all nodes that link to a specific node
- `create_node` - Create a new org-roam node
- `update_node` - Update content of an existing node
- `add_link` - Add a link from one node to another
- `list_files` - List all org files in the org-roam directory

## Installation

### Using Nix

```bash
nix build
```

### Using Cargo

```bash
cargo build --release
```

## Install (dev)

A justfile is provided:

```bash
just build       # release build
just install     # build + install to ~/bin/org-cli (on PATH)
just test        # run tests
just lint        # clippy
```

## CLI usage

```bash
org-cli agenda today
org-cli agenda upcoming --days 7
org-cli habits all
org-cli habits create "Weekly review" --repeater .+1w --file ~/Documents/org/habits.org --tags review
org-cli habits mark "Weekly review"
org-cli habits delete "Weekly review"
org-cli drill status
org-cli drill start
org-cli --mcp          # run as an MCP server (stdio)
```

## Configuration

Configuration file is located at `~/.config/org-mcp/config.toml` (the config directory name is still `org-mcp` for back-compat with existing installs):

```toml
[agenda]
files = [
    "~/Documents/org/roam/Inbox.org",
    "~/Documents/org/habits.org",
    "~/Documents/org/calendars/personal.org",
    "~/Documents/org/calendars/work.org",
]

[inbox]
file = "~/Documents/org/roam/Inbox.org"
sections = ["Personal", "Work", "Email"]

[refile]
projects = "~/Documents/org/roam/Projects.org"
areas = "~/Documents/org/roam/Areas.org"
resources = "~/Documents/org/roam/Resources.org"
archives = "~/Documents/org/roam/Archives.org"

[emacs]
use_emacsclient = true
socket_name = "server"  # optional
```

## Usage

### With Claude Desktop

Add to your Claude Desktop configuration:

```json
{
  "mcpServers": {
    "org-cli": {
      "command": "org-cli",
      "args": ["--mcp"]
    }
  }
}
```

### With Claude Code

Add to your Claude Code MCP settings:

```json
{
  "mcpServers": {
    "org-cli": {
      "command": "org-cli",
      "args": ["--mcp"]
    }
  }
}
```

## Requirements

- Emacs with org-mode
- org-roam (for org-roam features)
- Running Emacs server (`M-x server-start`)

## License

MIT
