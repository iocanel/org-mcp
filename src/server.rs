use std::path::PathBuf;
use std::sync::Arc;

use crate::config::Config;
use crate::emacs::{EmacsClient, EmacsClientTrait};
use crate::models::InboxSection;
use crate::roam::OrgRoamDatabase;
use crate::tools::{agenda, habits, inbox, tasks};
use anyhow::Result;
use rmcp::{
    ErrorData as McpError, RoleServer, ServerHandler,
    handler::server::router::tool::ToolRouter,
    model::*,
    schemars,
    service::RequestContext,
    tool, tool_handler, tool_router,
};
use rmcp::handler::server::tool::Parameters;

#[derive(Debug, Clone, serde::Deserialize, schemars::JsonSchema)]
pub struct UpcomingParams {
    #[schemars(description = "Number of days to look ahead (default: 7)")]
    pub days: Option<usize>,
}

#[derive(Debug, Clone, serde::Deserialize, schemars::JsonSchema)]
pub struct InboxQueryParams {
    #[schemars(description = "Section to filter by: 'personal', 'work', or 'email'")]
    pub section: Option<String>,
    #[schemars(description = "Include completed items (default: false)")]
    pub include_done: Option<bool>,
}

#[derive(Debug, Clone, serde::Deserialize, schemars::JsonSchema)]
pub struct AddInboxParams {
    #[schemars(description = "Title of the new item")]
    pub title: String,
    #[schemars(description = "Section to add to: 'personal', 'work', or 'email'")]
    pub section: String,
    #[schemars(description = "Optional body content")]
    pub body: Option<String>,
    #[schemars(description = "Optional scheduled date (YYYY-MM-DD format)")]
    pub scheduled: Option<String>,
    #[schemars(description = "Optional deadline date (YYYY-MM-DD format)")]
    pub deadline: Option<String>,
    #[schemars(description = "Optional tags")]
    pub tags: Option<Vec<String>>,
}

#[derive(Debug, Clone, serde::Deserialize, schemars::JsonSchema)]
pub struct HabitMarkParams {
    #[schemars(description = "ID or title of the habit to mark as done")]
    pub habit: String,
}

#[derive(Debug, Clone, serde::Deserialize, schemars::JsonSchema)]
pub struct CreateTaskParams {
    #[schemars(description = "Title of the new task")]
    pub title: String,
    #[schemars(description = "File path to create the task in")]
    pub file_path: String,
    #[schemars(description = "Optional scheduled date (YYYY-MM-DD format)")]
    pub scheduled: Option<String>,
    #[schemars(description = "Optional deadline date (YYYY-MM-DD format)")]
    pub deadline: Option<String>,
    #[schemars(description = "Optional tags")]
    pub tags: Option<Vec<String>>,
    #[schemars(description = "Optional body content")]
    pub body: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize, schemars::JsonSchema)]
pub struct TaskIdentifierParams {
    #[schemars(description = "ID or title of the task")]
    pub task: String,
}

#[derive(Debug, Clone, serde::Deserialize, schemars::JsonSchema)]
pub struct UpdateTaskScheduledParams {
    #[schemars(description = "ID or title of the task")]
    pub task: String,
    #[schemars(description = "New scheduled date (YYYY-MM-DD format)")]
    pub scheduled: String,
}

#[derive(Debug, Clone, serde::Deserialize, schemars::JsonSchema)]
pub struct UpdateTaskDeadlineParams {
    #[schemars(description = "ID or title of the task")]
    pub task: String,
    #[schemars(description = "New deadline date (YYYY-MM-DD format)")]
    pub deadline: String,
}

#[derive(Debug, Clone, serde::Deserialize, schemars::JsonSchema)]
pub struct RefileTaskParams {
    #[schemars(description = "ID or title of the task to refile")]
    pub task: String,
    #[schemars(description = "Target file path")]
    pub target_file: String,
    #[schemars(description = "Optional target heading within the file")]
    pub target_heading: Option<String>,
}

// Org-roam parameters
#[derive(Debug, Clone, serde::Deserialize, schemars::JsonSchema)]
pub struct SearchNodesParams {
    #[schemars(description = "Search query string")]
    pub query: String,
    #[schemars(description = "Maximum number of results (default: 50)")]
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, serde::Deserialize, schemars::JsonSchema)]
pub struct GetNodeParams {
    #[schemars(description = "The ID of the node to retrieve")]
    pub node_id: String,
}

#[derive(Debug, Clone, serde::Deserialize, schemars::JsonSchema)]
pub struct CreateNodeParams {
    #[schemars(description = "Title of the new node")]
    pub title: String,
    #[schemars(description = "Content of the new node")]
    pub content: Option<String>,
    #[schemars(description = "Tags for the new node")]
    pub tags: Option<Vec<String>>,
}

#[derive(Debug, Clone, serde::Deserialize, schemars::JsonSchema)]
pub struct UpdateNodeParams {
    #[schemars(description = "ID of the node to update")]
    pub node_id: String,
    #[schemars(description = "New content for the node")]
    pub content: String,
}

#[derive(Debug, Clone, serde::Deserialize, schemars::JsonSchema)]
pub struct AddLinkParams {
    #[schemars(description = "ID of the source node")]
    pub source_node_id: String,
    #[schemars(description = "ID of the target node")]
    pub target_node_id: String,
}

#[derive(Clone)]
pub struct OrgMcpServer {
    config: Arc<Config>,
    emacs: Arc<EmacsClient>,
    roam_db_path: Option<PathBuf>,
    tool_router: ToolRouter<OrgMcpServer>,
}

impl OrgMcpServer {
    pub fn new() -> Result<Self> {
        let config = Config::load()?;
        let emacs = if config.emacs.use_emacsclient {
            match &config.emacs.socket_name {
                Some(socket) => EmacsClient::with_socket(socket),
                None => EmacsClient::new(),
            }
        } else {
            EmacsClient::new()
        };

        // Try to find org-roam database path
        let roam_db_path = OrgRoamDatabase::find_database().ok();

        Ok(Self {
            config: Arc::new(config),
            emacs: Arc::new(emacs),
            roam_db_path,
            tool_router: Self::tool_router(),
        })
    }

    pub fn with_config(config: Config) -> Self {
        let emacs = EmacsClient::new();
        Self {
            config: Arc::new(config),
            emacs: Arc::new(emacs),
            roam_db_path: None,
            tool_router: Self::tool_router(),
        }
    }

    fn open_roam_db(&self) -> Result<OrgRoamDatabase, McpError> {
        let path = self
            .roam_db_path
            .as_ref()
            .ok_or_else(|| McpError::internal_error("Org-roam database not available", None))?;

        OrgRoamDatabase::open(path)
            .map_err(|e| McpError::internal_error(format!("Failed to open org-roam database: {}", e), None))
    }

    fn find_task(&self, task_identifier: &str) -> Result<crate::models::Task, McpError> {
        // Try finding by ID first
        if let Some(task) = tasks::find_task_by_id(&self.config, task_identifier)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?
        {
            return Ok(task);
        }

        // Try finding by title
        if let Some(task) = tasks::find_task_by_title(&self.config, task_identifier)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?
        {
            return Ok(task);
        }

        Err(McpError::invalid_params(
            format!("Task not found: {}", task_identifier),
            None,
        ))
    }
}

#[tool_router]
impl OrgMcpServer {
    #[tool(description = "Get today's agenda including tasks, habits, and events")]
    async fn get_agenda_today(&self) -> Result<CallToolResult, McpError> {
        let agenda = agenda::get_agenda_today(&self.config)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let json = serde_json::to_string_pretty(&agenda)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(description = "Get upcoming agenda items for the next N days")]
    async fn get_agenda_upcoming(
        &self,
        Parameters(params): Parameters<UpcomingParams>,
    ) -> Result<CallToolResult, McpError> {
        let days = params.days.unwrap_or(7);
        let agenda = agenda::get_agenda_upcoming(&self.config, days)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let json = serde_json::to_string_pretty(&agenda)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(description = "Query inbox items, optionally filtering by section")]
    async fn query_inbox(
        &self,
        Parameters(params): Parameters<InboxQueryParams>,
    ) -> Result<CallToolResult, McpError> {
        let section = params
            .section
            .as_ref()
            .and_then(|s| InboxSection::from_str(s));
        let include_done = params.include_done.unwrap_or(false);

        let items = inbox::query_inbox(&self.config, section, include_done)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let json = serde_json::to_string_pretty(&items)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(description = "Add a new item to the inbox under a specific section")]
    async fn add_to_inbox(
        &self,
        Parameters(params): Parameters<AddInboxParams>,
    ) -> Result<CallToolResult, McpError> {
        let section = InboxSection::from_str(&params.section)
            .ok_or_else(|| McpError::invalid_params("Invalid section. Use 'personal', 'work', or 'email'", None))?;

        let tags = params.tags.unwrap_or_default();

        inbox::add_to_inbox(
            &self.config,
            &*self.emacs,
            &params.title,
            section,
            params.body.as_deref(),
            params.scheduled.as_deref(),
            params.deadline.as_deref(),
            &tags,
        )
        .await
        .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(format!(
            "Added '{}' to {} inbox",
            params.title,
            section.as_str()
        ))]))
    }

    #[tool(description = "Get all habits with their current status")]
    async fn get_habits(&self) -> Result<CallToolResult, McpError> {
        let habits_list = habits::get_habits(&self.config)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let json = serde_json::to_string_pretty(&habits_list)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(description = "Get habits due today")]
    async fn get_habits_due_today(&self) -> Result<CallToolResult, McpError> {
        let habits_list = habits::get_habits_due_today(&self.config)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let json = serde_json::to_string_pretty(&habits_list)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(description = "Mark a habit as done for today")]
    async fn mark_habit_done(
        &self,
        Parameters(params): Parameters<HabitMarkParams>,
    ) -> Result<CallToolResult, McpError> {
        let habits_list = habits::get_habits(&self.config)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let habit = habits_list
            .iter()
            .find(|h| {
                h.id.as_ref().map_or(false, |id| id == &params.habit)
                    || h.title.eq_ignore_ascii_case(&params.habit)
            })
            .ok_or_else(|| McpError::invalid_params(format!("Habit not found: {}", params.habit), None))?;

        habits::mark_habit_done(&*self.emacs, habit)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(format!(
            "Marked '{}' as done",
            habit.title
        ))]))
    }

    #[tool(description = "Create a new task in the specified file")]
    async fn create_task(
        &self,
        Parameters(params): Parameters<CreateTaskParams>,
    ) -> Result<CallToolResult, McpError> {
        let tags = params.tags.unwrap_or_default();

        tasks::create_task(
            &*self.emacs,
            &params.file_path,
            &params.title,
            params.scheduled.as_deref(),
            params.deadline.as_deref(),
            &tags,
            params.body.as_deref(),
        )
        .await
        .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(format!(
            "Created task '{}' in {}",
            params.title, params.file_path
        ))]))
    }

    #[tool(description = "Mark a task as complete")]
    async fn complete_task(
        &self,
        Parameters(params): Parameters<TaskIdentifierParams>,
    ) -> Result<CallToolResult, McpError> {
        let task = self.find_task(&params.task)?;

        tasks::complete_task(&*self.emacs, &task)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(format!(
            "Completed task '{}'",
            task.title
        ))]))
    }

    #[tool(description = "Update the scheduled date of a task")]
    async fn update_task_scheduled(
        &self,
        Parameters(params): Parameters<UpdateTaskScheduledParams>,
    ) -> Result<CallToolResult, McpError> {
        let task = self.find_task(&params.task)?;

        tasks::update_task_scheduled(&*self.emacs, &task, &params.scheduled)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(format!(
            "Updated scheduled date for '{}' to {}",
            task.title, params.scheduled
        ))]))
    }

    #[tool(description = "Update the deadline of a task")]
    async fn update_task_deadline(
        &self,
        Parameters(params): Parameters<UpdateTaskDeadlineParams>,
    ) -> Result<CallToolResult, McpError> {
        let task = self.find_task(&params.task)?;

        tasks::update_task_deadline(&*self.emacs, &task, &params.deadline)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(format!(
            "Updated deadline for '{}' to {}",
            task.title, params.deadline
        ))]))
    }

    #[tool(description = "Refile a task to a different file or heading")]
    async fn refile_task(
        &self,
        Parameters(params): Parameters<RefileTaskParams>,
    ) -> Result<CallToolResult, McpError> {
        let task = self.find_task(&params.task)?;

        tasks::refile_task(
            &*self.emacs,
            &task,
            &params.target_file,
            params.target_heading.as_deref(),
        )
        .await
        .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let target_desc = if let Some(heading) = &params.target_heading {
            format!("{}::{}", params.target_file, heading)
        } else {
            params.target_file.clone()
        };

        Ok(CallToolResult::success(vec![Content::text(format!(
            "Refiled '{}' to {}",
            task.title, target_desc
        ))]))
    }

    // Org-roam tools
    #[tool(description = "Search for nodes by title, tags, or aliases")]
    async fn search_nodes(
        &self,
        Parameters(params): Parameters<SearchNodesParams>,
    ) -> Result<CallToolResult, McpError> {
        let db = self.open_roam_db()?;
        let limit = params.limit.unwrap_or(50);

        let nodes = db
            .search_nodes(&params.query, Some(limit))
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let results: Vec<_> = nodes
            .into_iter()
            .map(|n| {
                serde_json::json!({
                    "id": n.id,
                    "title": n.title,
                    "file": n.file,
                    "tags": n.tags,
                    "aliases": n.aliases,
                })
            })
            .collect();

        let response = serde_json::json!({
            "query": params.query,
            "results": results,
            "count": results.len()
        });

        let json = serde_json::to_string_pretty(&response)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(description = "Get detailed information about a specific node")]
    async fn get_node(
        &self,
        Parameters(params): Parameters<GetNodeParams>,
    ) -> Result<CallToolResult, McpError> {
        let db = self.open_roam_db()?;

        let node = db
            .get_node_by_id(&params.node_id)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?
            .ok_or_else(|| {
                McpError::invalid_params(format!("Node not found: {}", params.node_id), None)
            })?;

        let backlinks = db
            .get_backlinks(&params.node_id)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let forward_links = db
            .get_forward_links(&params.node_id)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        // Try to read file content
        let content = if std::path::Path::new(&node.file).exists() {
            std::fs::read_to_string(&node.file).unwrap_or_default()
        } else {
            String::new()
        };

        let result = serde_json::json!({
            "id": node.id,
            "title": node.title,
            "file": node.file,
            "level": node.level,
            "content": content,
            "tags": node.tags,
            "aliases": node.aliases,
            "backlinks_count": backlinks.len(),
            "forward_links_count": forward_links.len(),
        });

        let json = serde_json::to_string_pretty(&result)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(description = "Get all nodes that link to a specific node")]
    async fn get_backlinks(
        &self,
        Parameters(params): Parameters<GetNodeParams>,
    ) -> Result<CallToolResult, McpError> {
        let db = self.open_roam_db()?;

        let backlinks = db
            .get_backlinks(&params.node_id)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let mut results = Vec::new();
        for link in backlinks {
            if let Ok(Some(source_node)) = db.get_node_by_id(&link.source) {
                results.push(serde_json::json!({
                    "source_id": source_node.id,
                    "source_title": source_node.title,
                    "source_file": source_node.file,
                    "link_type": link.link_type,
                }));
            }
        }

        let response = serde_json::json!({
            "target_node": params.node_id,
            "backlinks": results,
            "count": results.len()
        });

        let json = serde_json::to_string_pretty(&response)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(description = "Create a new org-roam node")]
    async fn create_node(
        &self,
        Parameters(params): Parameters<CreateNodeParams>,
    ) -> Result<CallToolResult, McpError> {
        let tags = params.tags.unwrap_or_default();
        let content = params.content.unwrap_or_default();

        // Generate a unique ID
        let node_id = uuid::Uuid::new_v4().to_string();

        // Build the org-roam file content
        let tags_str = if tags.is_empty() {
            String::new()
        } else {
            format!("#+filetags: :{}: \n", tags.join(":"))
        };

        let file_content = format!(
            ":PROPERTIES:\n:ID:       {}\n:END:\n#+title: {}\n{}\n{}",
            node_id, params.title, tags_str, content
        );

        // Use emacsclient to create the file
        let roam_dir = dirs::home_dir()
            .map(|h| h.join("Documents/org/roam"))
            .ok_or_else(|| McpError::internal_error("Could not determine home directory", None))?;

        let filename = format!(
            "{}.org",
            params
                .title
                .to_lowercase()
                .replace(' ', "-")
                .replace(|c: char| !c.is_alphanumeric() && c != '-', "")
        );
        let file_path = roam_dir.join(&filename);

        let elisp = format!(
            r#"(progn
  (with-temp-file "{}"
    (insert "{}"))
  (org-roam-db-sync))"#,
            file_path.display(),
            file_content.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n")
        );

        self.emacs
            .eval(&elisp)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&serde_json::json!({
                "success": true,
                "node_id": node_id,
                "title": params.title,
                "file": file_path.display().to_string(),
                "message": format!("Created new node: {}", params.title)
            }))
            .unwrap(),
        )]))
    }

    #[tool(description = "Update content of an existing node")]
    async fn update_node(
        &self,
        Parameters(params): Parameters<UpdateNodeParams>,
    ) -> Result<CallToolResult, McpError> {
        let db = self.open_roam_db()?;

        let node = db
            .get_node_by_id(&params.node_id)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?
            .ok_or_else(|| {
                McpError::invalid_params(format!("Node not found: {}", params.node_id), None)
            })?;

        // Use emacsclient to update the file
        let elisp = format!(
            r#"(with-current-buffer (find-file-noselect "{}")
  (goto-char (point-min))
  (when (re-search-forward "^#\\+title:.*$" nil t)
    (forward-line 1)
    (delete-region (point) (point-max))
    (insert "\n{}"))
  (save-buffer)
  (org-roam-db-sync))"#,
            node.file,
            params.content.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n")
        );

        self.emacs
            .eval(&elisp)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&serde_json::json!({
                "success": true,
                "node_id": params.node_id,
                "message": format!("Updated node: {}", node.title.unwrap_or_default())
            }))
            .unwrap(),
        )]))
    }

    #[tool(description = "Add a link from one node to another")]
    async fn add_link(
        &self,
        Parameters(params): Parameters<AddLinkParams>,
    ) -> Result<CallToolResult, McpError> {
        let db = self.open_roam_db()?;

        let source_node = db
            .get_node_by_id(&params.source_node_id)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?
            .ok_or_else(|| {
                McpError::invalid_params(
                    format!("Source node not found: {}", params.source_node_id),
                    None,
                )
            })?;

        let target_node = db
            .get_node_by_id(&params.target_node_id)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?
            .ok_or_else(|| {
                McpError::invalid_params(
                    format!("Target node not found: {}", params.target_node_id),
                    None,
                )
            })?;

        let target_title = target_node.title.clone().unwrap_or_else(|| "Untitled".to_string());

        // Use emacsclient to add the link
        let elisp = format!(
            r#"(with-current-buffer (find-file-noselect "{}")
  (goto-char (point-max))
  (insert "\n[[id:{}][{}]]")
  (save-buffer)
  (org-roam-db-sync))"#,
            source_node.file,
            params.target_node_id,
            target_title
        );

        self.emacs
            .eval(&elisp)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&serde_json::json!({
                "success": true,
                "source_node": source_node.title,
                "target_node": target_title,
                "message": format!("Added link from '{}' to '{}'", source_node.title.unwrap_or_default(), target_title)
            }))
            .unwrap(),
        )]))
    }

    #[tool(description = "List all org files in the org-roam directory")]
    async fn list_files(&self) -> Result<CallToolResult, McpError> {
        let db = self.open_roam_db()?;

        let files = db
            .get_all_files()
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let files_info: Vec<_> = files
            .into_iter()
            .take(50) // Limit to avoid too much data
            .map(|f| {
                serde_json::json!({
                    "file": f.file,
                    "title": f.title,
                })
            })
            .collect();

        let response = serde_json::json!({
            "files": files_info,
            "displayed_count": files_info.len()
        });

        let json = serde_json::to_string_pretty(&response)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(json)]))
    }
}

#[tool_handler]
impl ServerHandler for OrgMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2024_11_05,
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .build(),
            server_info: Implementation {
                name: "org-mcp".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
            instructions: Some("MCP server for org-agenda integration. Query tasks, habits, and events from your org files.".to_string()),
        }
    }

    async fn initialize(
        &self,
        _request: InitializeRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> Result<InitializeResult, McpError> {
        Ok(self.get_info())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_server_info() {
        let config = Config::default();
        let server = OrgMcpServer::with_config(config);
        let info = server.get_info();

        assert_eq!(info.server_info.name, "org-mcp");
        assert!(!info.server_info.version.is_empty());
    }

    #[test]
    fn test_inbox_section_parsing() {
        assert!(InboxSection::from_str("personal").is_some());
        assert!(InboxSection::from_str("work").is_some());
        assert!(InboxSection::from_str("email").is_some());
        assert!(InboxSection::from_str("invalid").is_none());
    }
}
