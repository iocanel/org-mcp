use anyhow::Result;
use clap::{Parser, Subcommand};
use chrono::Local;
use org_cli::config::Config;
use org_cli::emacs::EmacsClient;
use org_cli::server::OrgMcpServer;
use org_cli::tools::{agenda, habits};
use rmcp::{ServiceExt, transport::stdio};
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(name = "org-cli")]
#[command(about = "CLI and MCP server for org-agenda integration")]
#[command(version)]
struct Cli {
    /// Run as MCP server (stdio transport)
    #[arg(long)]
    mcp: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Get agenda information
    Agenda {
        #[command(subcommand)]
        subcommand: AgendaCommands,
    },
    /// Get habits information
    Habits {
        #[command(subcommand)]
        subcommand: HabitsCommands,
    },
}

#[derive(Subcommand)]
enum AgendaCommands {
    /// Get today's agenda
    Today,
    /// Get upcoming agenda for next N days
    Upcoming {
        /// Number of days to look ahead
        #[arg(short, long, default_value = "7")]
        days: usize,
    },
}

#[derive(Subcommand)]
enum HabitsCommands {
    /// Get all habits
    All,
    /// Get habits due today
    Today,
    /// Create a new habit
    Create {
        /// Habit title
        title: String,
        /// Repeater, e.g. .+1d, .+1w, .+1m
        #[arg(short, long, default_value = ".+1d")]
        repeater: String,
        /// First scheduled date (YYYY-MM-DD), defaults to today
        #[arg(short, long)]
        scheduled: Option<String>,
        /// File to create the habit in, defaults to the first agenda file
        #[arg(short, long)]
        file: Option<String>,
        /// Tags
        #[arg(short, long)]
        tags: Vec<String>,
    },
    /// Mark a habit as done for today (org-habit reschedules it)
    Mark {
        /// Habit ID or title
        habit: String,
    },
    /// Delete a habit by ID or title
    Delete {
        /// Habit ID or title
        habit: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    if cli.mcp {
        run_mcp_server().await
    } else {
        match cli.command {
            Some(cmd) => run_cli_command(cmd).await,
            None => {
                eprintln!("No command specified. Use --help for usage or --mcp to run as MCP server.");
                std::process::exit(1);
            }
        }
    }
}

async fn run_mcp_server() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive(tracing::Level::INFO.into()))
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();

    tracing::info!("Starting org-cli MCP server");

    let server = OrgMcpServer::new()?;
    let service = server.serve(stdio()).await.inspect_err(|e| {
        tracing::error!("serving error: {:?}", e);
    })?;

    service.waiting().await?;
    Ok(())
}

async fn run_cli_command(cmd: Commands) -> Result<()> {
    let config = Config::load()?;

    match cmd {
        Commands::Agenda { subcommand } => match subcommand {
            AgendaCommands::Today => {
                let result = agenda::get_agenda_today(&config)?;
                println!("{}", serde_json::to_string_pretty(&result)?);
            }
            AgendaCommands::Upcoming { days } => {
                let result = agenda::get_agenda_upcoming(&config, days)?;
                println!("{}", serde_json::to_string_pretty(&result)?);
            }
        },
        Commands::Habits { subcommand } => match subcommand {
            HabitsCommands::All => {
                let result = habits::get_habits(&config)?;
                println!("{}", serde_json::to_string_pretty(&result)?);
            }
            HabitsCommands::Today => {
                let result = habits::get_habits_due_today(&config)?;
                println!("{}", serde_json::to_string_pretty(&result)?);
            }
            HabitsCommands::Create {
                title,
                repeater,
                scheduled,
                file,
                tags,
            } => {
                let emacs = EmacsClient::new();
                let scheduled = scheduled
                    .unwrap_or_else(|| Local::now().date_naive().format("%Y-%m-%d").to_string());
                let file = file.unwrap_or_else(|| {
                    config
                        .agenda_files()
                        .first()
                        .map(|p| p.display().to_string())
                        .unwrap_or_else(|| "~/Documents/org/habits.org".to_string())
                });
                habits::create_habit(&emacs, &file, &title, &scheduled, &repeater, &tags).await?;
                println!("Created habit '{}' in {}", title, file);
            }
            HabitsCommands::Mark { habit } => {
                let emacs = EmacsClient::new();
                let target = find_habit(&config, &habit)?;
                habits::mark_habit_done(&emacs, &target).await?;
                println!("Marked '{}' as done", target.title);
            }
            HabitsCommands::Delete { habit } => {
                let emacs = EmacsClient::new();
                let target = find_habit(&config, &habit)?;
                habits::delete_habit(&emacs, &target).await?;
                println!("Deleted '{}'", target.title);
            }
        },
    }

    Ok(())
}

fn find_habit(config: &Config, identifier: &str) -> Result<org_cli::models::Habit> {
    let habits_list = habits::get_habits(config)?;
    habits_list
        .into_iter()
        .find(|h| {
            h.id.as_ref().map_or(false, |id| id == identifier)
                || h.title.eq_ignore_ascii_case(identifier)
        })
        .ok_or_else(|| anyhow::anyhow!("Habit not found: {}", identifier))
}
