use crate::config::Config;
use crate::emacs::EmacsClientTrait;
use crate::models::Task;
use crate::parser::OrgFile;
use anyhow::Result;

pub async fn create_task<E: EmacsClientTrait>(
    emacs: &E,
    file_path: &str,
    title: &str,
    scheduled: Option<&str>,
    deadline: Option<&str>,
    tags: &[String],
    body: Option<&str>,
) -> Result<()> {
    let mut headline = format!("* TODO {}", title);

    if !tags.is_empty() {
        headline.push_str(&format!(" :{}:", tags.join(":")));
    }

    let mut content_parts = vec![headline];

    if let Some(sched) = scheduled {
        content_parts.push(format!("SCHEDULED: <{}>", sched));
    }

    if let Some(dead) = deadline {
        content_parts.push(format!("DEADLINE: <{}>", dead));
    }

    if let Some(b) = body {
        content_parts.push(b.to_string());
    }

    let content = content_parts.join("\n");

    let elisp = format!(
        r#"(with-current-buffer (find-file-noselect "{}")
  (goto-char (point-max))
  (insert "\n{}\n")
  (save-buffer))"#,
        file_path,
        content.replace('\\', "\\\\").replace('"', "\\\"")
    );

    emacs.eval(&elisp).await?;
    Ok(())
}

pub async fn complete_task<E: EmacsClientTrait>(
    emacs: &E,
    task: &Task,
) -> Result<()> {
    let elisp = format!(
        r#"(with-current-buffer (find-file-noselect "{}")
  (goto-char (point-min))
  (when (re-search-forward "^\\*+ TODO {}" nil t)
    (org-todo 'done))
  (save-buffer))"#,
        task.file_path,
        task.title.replace('\\', "\\\\").replace('"', "\\\"")
    );

    emacs.eval(&elisp).await?;
    Ok(())
}

pub async fn update_task_scheduled<E: EmacsClientTrait>(
    emacs: &E,
    task: &Task,
    scheduled: &str,
) -> Result<()> {
    let elisp = format!(
        r#"(with-current-buffer (find-file-noselect "{}")
  (goto-char (point-min))
  (when (re-search-forward "^\\*+ \\(TODO\\|NEXT\\|WAITING\\) {}" nil t)
    (org-schedule nil "<{}>"))
  (save-buffer))"#,
        task.file_path,
        task.title.replace('\\', "\\\\").replace('"', "\\\""),
        scheduled
    );

    emacs.eval(&elisp).await?;
    Ok(())
}

pub async fn update_task_deadline<E: EmacsClientTrait>(
    emacs: &E,
    task: &Task,
    deadline: &str,
) -> Result<()> {
    let elisp = format!(
        r#"(with-current-buffer (find-file-noselect "{}")
  (goto-char (point-min))
  (when (re-search-forward "^\\*+ \\(TODO\\|NEXT\\|WAITING\\) {}" nil t)
    (org-deadline nil "<{}>"))
  (save-buffer))"#,
        task.file_path,
        task.title.replace('\\', "\\\\").replace('"', "\\\""),
        deadline
    );

    emacs.eval(&elisp).await?;
    Ok(())
}

pub async fn refile_task<E: EmacsClientTrait>(
    emacs: &E,
    task: &Task,
    target_file: &str,
    target_heading: Option<&str>,
) -> Result<()> {
    let target = if let Some(heading) = target_heading {
        format!(r#"'("{}" "{}")"#, target_file, heading)
    } else {
        format!(r#"'("{}" nil)"#, target_file)
    };

    let elisp = format!(
        r#"(with-current-buffer (find-file-noselect "{}")
  (goto-char (point-min))
  (when (re-search-forward "^\\*+ \\(TODO\\|DONE\\|NEXT\\|WAITING\\|CANCELLED\\) {}" nil t)
    (org-refile nil nil {}))
  (save-buffer)
  (with-current-buffer (find-file-noselect "{}")
    (save-buffer)))"#,
        task.file_path,
        task.title.replace('\\', "\\\\").replace('"', "\\\""),
        target,
        target_file
    );

    emacs.eval(&elisp).await?;
    Ok(())
}

pub fn find_task_by_id(config: &Config, task_id: &str) -> Result<Option<Task>> {
    for file_path in config.agenda_files() {
        if !file_path.exists() {
            continue;
        }

        let org = OrgFile::parse(&file_path)?;
        let file_str = file_path.display().to_string();

        if let Some(headline) = org.find_headline_by_id(task_id) {
            return Ok(Some(Task::from_headline(headline, &file_str)));
        }
    }

    // Also check inbox file
    let inbox_path = config.inbox_file();
    if inbox_path.exists() {
        let org = OrgFile::parse(&inbox_path)?;
        let file_str = inbox_path.display().to_string();

        if let Some(headline) = org.find_headline_by_id(task_id) {
            return Ok(Some(Task::from_headline(headline, &file_str)));
        }
    }

    Ok(None)
}

pub fn find_task_by_title(config: &Config, title: &str) -> Result<Option<Task>> {
    for file_path in config.agenda_files() {
        if !file_path.exists() {
            continue;
        }

        let org = OrgFile::parse(&file_path)?;
        let file_str = file_path.display().to_string();

        for headline in &org.headlines {
            if headline.title.eq_ignore_ascii_case(title) {
                return Ok(Some(Task::from_headline(headline, &file_str)));
            }
        }
    }

    // Also check inbox file
    let inbox_path = config.inbox_file();
    if inbox_path.exists() {
        let org = OrgFile::parse(&inbox_path)?;
        let file_str = inbox_path.display().to_string();

        for headline in &org.headlines {
            if headline.title.eq_ignore_ascii_case(title) {
                return Ok(Some(Task::from_headline(headline, &file_str)));
            }
        }
    }

    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::emacs::MockEmacsClientTrait;
    use crate::parser::TodoState;
    use chrono::NaiveDate;
    use tempfile::TempDir;

    fn create_test_task() -> Task {
        Task {
            id: Some("test-task-id".to_string()),
            title: "Test task".to_string(),
            state: Some(TodoState::Todo),
            priority: None,
            tags: vec![],
            scheduled: Some(NaiveDate::from_ymd_opt(2026, 3, 6).unwrap()),
            deadline: None,
            body: String::new(),
            file_path: "/path/to/tasks.org".to_string(),
            line_number: 10,
        }
    }

    fn create_test_config(temp_dir: &TempDir) -> Config {
        let tasks_path = temp_dir.path().join("tasks.org");
        let inbox_path = temp_dir.path().join("inbox.org");

        let tasks_content = r#"#+title: Tasks
* Projects
** TODO Project task
:PROPERTIES:
:ID:       project-task-id
:END:
** TODO Another task
:PROPERTIES:
:ID:       another-task-id
:END:"#;
        std::fs::write(&tasks_path, tasks_content).unwrap();

        let inbox_content = r#"#+title: Inbox
* Personal
** TODO Inbox task
:PROPERTIES:
:ID:       inbox-task-id
:END:"#;
        std::fs::write(&inbox_path, inbox_content).unwrap();

        Config {
            agenda: crate::config::AgendaConfig {
                files: vec![tasks_path.display().to_string()],
            },
            inbox: crate::config::InboxConfig {
                file: inbox_path.display().to_string(),
                sections: vec!["Personal".to_string()],
            },
            refile: crate::config::RefileConfig {
                projects: String::new(),
                areas: String::new(),
                resources: String::new(),
                archives: String::new(),
            },
            emacs: crate::config::EmacsConfig {
                use_emacsclient: false,
                socket_name: None,
            },
        }
    }

    #[tokio::test]
    async fn test_create_task_basic() {
        let mut mock_emacs = MockEmacsClientTrait::new();
        mock_emacs
            .expect_eval()
            .withf(|elisp: &str| {
                elisp.contains("find-file-noselect") &&
                elisp.contains("/path/to/tasks.org") &&
                elisp.contains("* TODO New task")
            })
            .times(1)
            .returning(|_| Box::pin(async { Ok("nil".to_string()) }));

        let result = create_task(
            &mock_emacs,
            "/path/to/tasks.org",
            "New task",
            None,
            None,
            &[],
            None,
        ).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_create_task_with_scheduled() {
        let mut mock_emacs = MockEmacsClientTrait::new();
        mock_emacs
            .expect_eval()
            .withf(|elisp: &str| {
                elisp.contains("SCHEDULED: <2026-03-10>")
            })
            .times(1)
            .returning(|_| Box::pin(async { Ok("nil".to_string()) }));

        let result = create_task(
            &mock_emacs,
            "/path/to/tasks.org",
            "Scheduled task",
            Some("2026-03-10"),
            None,
            &[],
            None,
        ).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_create_task_with_deadline() {
        let mut mock_emacs = MockEmacsClientTrait::new();
        mock_emacs
            .expect_eval()
            .withf(|elisp: &str| {
                elisp.contains("DEADLINE: <2026-03-15>")
            })
            .times(1)
            .returning(|_| Box::pin(async { Ok("nil".to_string()) }));

        let result = create_task(
            &mock_emacs,
            "/path/to/tasks.org",
            "Deadline task",
            None,
            Some("2026-03-15"),
            &[],
            None,
        ).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_create_task_with_tags() {
        let mut mock_emacs = MockEmacsClientTrait::new();
        mock_emacs
            .expect_eval()
            .withf(|elisp: &str| {
                elisp.contains(":urgent:project:")
            })
            .times(1)
            .returning(|_| Box::pin(async { Ok("nil".to_string()) }));

        let result = create_task(
            &mock_emacs,
            "/path/to/tasks.org",
            "Tagged task",
            None,
            None,
            &["urgent".to_string(), "project".to_string()],
            None,
        ).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_create_task_with_body() {
        let mut mock_emacs = MockEmacsClientTrait::new();
        mock_emacs
            .expect_eval()
            .withf(|elisp: &str| {
                elisp.contains("This is the task description")
            })
            .times(1)
            .returning(|_| Box::pin(async { Ok("nil".to_string()) }));

        let result = create_task(
            &mock_emacs,
            "/path/to/tasks.org",
            "Task with body",
            None,
            None,
            &[],
            Some("This is the task description"),
        ).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_create_task_full_options() {
        let mut mock_emacs = MockEmacsClientTrait::new();
        mock_emacs
            .expect_eval()
            .withf(|elisp: &str| {
                elisp.contains("* TODO Full task") &&
                elisp.contains(":tag1:tag2:") &&
                elisp.contains("SCHEDULED: <2026-03-10>") &&
                elisp.contains("DEADLINE: <2026-03-20>") &&
                elisp.contains("Task body")
            })
            .times(1)
            .returning(|_| Box::pin(async { Ok("nil".to_string()) }));

        let result = create_task(
            &mock_emacs,
            "/path/to/tasks.org",
            "Full task",
            Some("2026-03-10"),
            Some("2026-03-20"),
            &["tag1".to_string(), "tag2".to_string()],
            Some("Task body"),
        ).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_complete_task() {
        let task = create_test_task();

        let mut mock_emacs = MockEmacsClientTrait::new();
        mock_emacs
            .expect_eval()
            .withf(|elisp: &str| {
                elisp.contains("find-file-noselect") &&
                elisp.contains("/path/to/tasks.org") &&
                elisp.contains("Test task") &&
                elisp.contains("org-todo")
            })
            .times(1)
            .returning(|_| Box::pin(async { Ok("nil".to_string()) }));

        let result = complete_task(&mock_emacs, &task).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_complete_task_with_special_chars() {
        let mut task = create_test_task();
        task.title = r#"Task with "quotes""#.to_string();

        let mut mock_emacs = MockEmacsClientTrait::new();
        mock_emacs
            .expect_eval()
            .withf(|elisp: &str| {
                elisp.contains(r#"\"quotes\""#)
            })
            .times(1)
            .returning(|_| Box::pin(async { Ok("nil".to_string()) }));

        let result = complete_task(&mock_emacs, &task).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_update_task_scheduled() {
        let task = create_test_task();

        let mut mock_emacs = MockEmacsClientTrait::new();
        mock_emacs
            .expect_eval()
            .withf(|elisp: &str| {
                elisp.contains("org-schedule") &&
                elisp.contains("2026-03-15")
            })
            .times(1)
            .returning(|_| Box::pin(async { Ok("nil".to_string()) }));

        let result = update_task_scheduled(&mock_emacs, &task, "2026-03-15").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_update_task_deadline() {
        let task = create_test_task();

        let mut mock_emacs = MockEmacsClientTrait::new();
        mock_emacs
            .expect_eval()
            .withf(|elisp: &str| {
                elisp.contains("org-deadline") &&
                elisp.contains("2026-03-20")
            })
            .times(1)
            .returning(|_| Box::pin(async { Ok("nil".to_string()) }));

        let result = update_task_deadline(&mock_emacs, &task, "2026-03-20").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_refile_task_to_file() {
        let task = create_test_task();

        let mut mock_emacs = MockEmacsClientTrait::new();
        mock_emacs
            .expect_eval()
            .withf(|elisp: &str| {
                elisp.contains("org-refile") &&
                elisp.contains("/path/to/projects.org")
            })
            .times(1)
            .returning(|_| Box::pin(async { Ok("nil".to_string()) }));

        let result = refile_task(&mock_emacs, &task, "/path/to/projects.org", None).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_refile_task_to_heading() {
        let task = create_test_task();

        let mut mock_emacs = MockEmacsClientTrait::new();
        mock_emacs
            .expect_eval()
            .withf(|elisp: &str| {
                elisp.contains("org-refile") &&
                elisp.contains("/path/to/projects.org") &&
                elisp.contains("Active Projects")
            })
            .times(1)
            .returning(|_| Box::pin(async { Ok("nil".to_string()) }));

        let result = refile_task(
            &mock_emacs,
            &task,
            "/path/to/projects.org",
            Some("Active Projects"),
        ).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_refile_task_emacs_error() {
        let task = create_test_task();

        let mut mock_emacs = MockEmacsClientTrait::new();
        mock_emacs
            .expect_eval()
            .times(1)
            .returning(|_| Box::pin(async { Err(anyhow::anyhow!("Refile failed")) }));

        let result = refile_task(&mock_emacs, &task, "/path/to/projects.org", None).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_find_task_by_id_in_agenda() {
        let temp_dir = TempDir::new().unwrap();
        let config = create_test_config(&temp_dir);

        let task = find_task_by_id(&config, "project-task-id").unwrap();
        assert!(task.is_some());
        assert_eq!(task.unwrap().title, "Project task");
    }

    #[test]
    fn test_find_task_by_id_in_inbox() {
        let temp_dir = TempDir::new().unwrap();
        let config = create_test_config(&temp_dir);

        let task = find_task_by_id(&config, "inbox-task-id").unwrap();
        assert!(task.is_some());
        assert_eq!(task.unwrap().title, "Inbox task");
    }

    #[test]
    fn test_find_task_by_id_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let config = create_test_config(&temp_dir);

        let task = find_task_by_id(&config, "nonexistent-id").unwrap();
        assert!(task.is_none());
    }

    #[test]
    fn test_find_task_by_title() {
        let temp_dir = TempDir::new().unwrap();
        let config = create_test_config(&temp_dir);

        let task = find_task_by_title(&config, "Project task").unwrap();
        assert!(task.is_some());
        assert_eq!(task.unwrap().id, Some("project-task-id".to_string()));
    }

    #[test]
    fn test_find_task_by_title_case_insensitive() {
        let temp_dir = TempDir::new().unwrap();
        let config = create_test_config(&temp_dir);

        let task = find_task_by_title(&config, "PROJECT TASK").unwrap();
        assert!(task.is_some());
    }

    #[test]
    fn test_find_task_by_title_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let config = create_test_config(&temp_dir);

        let task = find_task_by_title(&config, "Nonexistent task").unwrap();
        assert!(task.is_none());
    }
}
