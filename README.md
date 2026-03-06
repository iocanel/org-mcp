# org-mcp

An MCP (Model Context Protocol) server for org-mode and org-roam integration.

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
- `mark_habit_done` - Mark a habit as done for today

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

## Configuration

Configuration file is located at `~/.config/org-mcp/config.toml`:

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
    "org-mcp": {
      "command": "/path/to/org-mcp"
    }
  }
}
```

### With Claude Code

Add to your Claude Code MCP settings:

```json
{
  "mcpServers": {
    "org-mcp": {
      "command": "/path/to/org-mcp"
    }
  }
}
```

## Requirements

- Emacs with org-mode
- org-roam (for org-roam features)
- Running Emacs server (`M-x server-start`)

## License

Apache License 2.0
