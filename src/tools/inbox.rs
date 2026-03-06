use crate::config::Config;
use crate::emacs::EmacsClientTrait;
use crate::models::{InboxItem, InboxSection};
use crate::parser::OrgFile;
use anyhow::{Context, Result};

pub fn query_inbox(config: &Config, section: Option<InboxSection>, include_done: bool) -> Result<Vec<InboxItem>> {
    let inbox_path = config.inbox_file();
    let org = OrgFile::parse(&inbox_path)
        .with_context(|| format!("Failed to parse inbox file: {}", inbox_path.display()))?;

    let mut items = Vec::new();

    for section_name in &config.inbox.sections {
        let current_section = match InboxSection::from_str(section_name) {
            Some(s) => s,
            None => continue,
        };

        // Filter by section if specified
        if let Some(filter_section) = section {
            if current_section != filter_section {
                continue;
            }
        }

        // Find the section headline
        if let Some(section_headline) = org.find_section(section_name) {
            // Get all children (tasks) under this section
            let children = org.get_all_descendants(section_headline);

            for child in children {
                // Skip non-task headlines (e.g., "Follow Up", "Read Later" subsections)
                if child.todo_state.is_none() {
                    continue;
                }

                // Skip done items unless include_done is true
                if !include_done && child.is_done() {
                    continue;
                }

                let item = InboxItem::from_headline(child, current_section);
                items.push(item);
            }
        }
    }

    Ok(items)
}

pub async fn add_to_inbox<E: EmacsClientTrait>(
    config: &Config,
    emacs: &E,
    title: &str,
    section: InboxSection,
    body: Option<&str>,
    scheduled: Option<&str>,
    deadline: Option<&str>,
    tags: &[String],
) -> Result<()> {
    let inbox_file = config.inbox_file();
    let inbox_file_str = inbox_file.display().to_string();
    let section_name = section.as_str();

    // Build the headline content
    let mut headline = format!("** TODO {}", title);

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

    // Use emacsclient to add the item under the correct section
    let elisp = format!(
        r#"(with-current-buffer (find-file-noselect "{}")
  (goto-char (point-min))
  (when (re-search-forward "^\\* {}\\b" nil t)
    (org-end-of-subtree t t)
    (insert "\n{}\n"))
  (save-buffer))"#,
        inbox_file_str,
        section_name,
        content.replace('\\', "\\\\").replace('"', "\\\"")
    );

    emacs.eval(&elisp).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::emacs::MockEmacsClientTrait;
    use tempfile::TempDir;

    fn create_test_inbox(temp_dir: &TempDir) -> (Config, std::path::PathBuf) {
        let inbox_path = temp_dir.path().join("Inbox.org");

        let inbox_content = r#"#+title: Inbox

Capture items here.

* Personal :personal:
** TODO Personal task one
SCHEDULED: <2026-03-06 Fri>
:PROPERTIES:
:ID:       personal-1
:END:
** TODO Personal task two
:PROPERTIES:
:ID:       personal-2
:END:
** DONE Completed personal task
:PROPERTIES:
:ID:       personal-done
:END:

* Work :work:
** TODO Work task
SCHEDULED: <2026-03-07 Sat>
:PROPERTIES:
:ID:       work-1
:END:
** TODO Another work task
:PROPERTIES:
:ID:       work-2
:END:

* Email :email:
** Follow Up
*** TODO Follow up email
DEADLINE: <2026-03-08 Sun>
:PROPERTIES:
:ID:       email-followup
:END:
** Read Later
*** TODO Article to read
:PROPERTIES:
:ID:       email-read
:END:
"#;
        std::fs::write(&inbox_path, inbox_content).unwrap();

        let config = Config {
            agenda: crate::config::AgendaConfig { files: vec![] },
            inbox: crate::config::InboxConfig {
                file: inbox_path.display().to_string(),
                sections: vec![
                    "Personal".to_string(),
                    "Work".to_string(),
                    "Email".to_string(),
                ],
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
        };

        (config, inbox_path)
    }

    #[test]
    fn test_query_inbox_all() {
        let temp_dir = TempDir::new().unwrap();
        let (config, _) = create_test_inbox(&temp_dir);

        let items = query_inbox(&config, None, false).unwrap();

        // Should have: 2 personal + 2 work + 2 email = 6 items (excluding done)
        assert_eq!(items.len(), 6);
    }

    #[test]
    fn test_query_inbox_with_done() {
        let temp_dir = TempDir::new().unwrap();
        let (config, _) = create_test_inbox(&temp_dir);

        let items = query_inbox(&config, None, true).unwrap();

        // Should have 7 items including the completed one
        assert_eq!(items.len(), 7);
        assert!(items.iter().any(|i| i.title == "Completed personal task"));
    }

    #[test]
    fn test_query_inbox_personal_only() {
        let temp_dir = TempDir::new().unwrap();
        let (config, _) = create_test_inbox(&temp_dir);

        let items = query_inbox(&config, Some(InboxSection::Personal), false).unwrap();

        assert_eq!(items.len(), 2);
        assert!(items.iter().all(|i| i.section == InboxSection::Personal));
    }

    #[test]
    fn test_query_inbox_work_only() {
        let temp_dir = TempDir::new().unwrap();
        let (config, _) = create_test_inbox(&temp_dir);

        let items = query_inbox(&config, Some(InboxSection::Work), false).unwrap();

        assert_eq!(items.len(), 2);
        assert!(items.iter().any(|i| i.title == "Work task"));
        assert!(items.iter().any(|i| i.title == "Another work task"));
    }

    #[test]
    fn test_query_inbox_email_only() {
        let temp_dir = TempDir::new().unwrap();
        let (config, _) = create_test_inbox(&temp_dir);

        let items = query_inbox(&config, Some(InboxSection::Email), false).unwrap();

        assert_eq!(items.len(), 2);
        assert!(items.iter().any(|i| i.title == "Follow up email"));
        assert!(items.iter().any(|i| i.title == "Article to read"));
    }

    #[test]
    fn test_query_inbox_item_properties() {
        let temp_dir = TempDir::new().unwrap();
        let (config, _) = create_test_inbox(&temp_dir);

        let items = query_inbox(&config, Some(InboxSection::Personal), false).unwrap();

        let task_one = items.iter().find(|i| i.title == "Personal task one").unwrap();
        assert_eq!(task_one.id, Some("personal-1".to_string()));
        assert_eq!(
            task_one.scheduled,
            Some(chrono::NaiveDate::from_ymd_opt(2026, 3, 6).unwrap())
        );
    }

    #[tokio::test]
    async fn test_add_to_inbox_basic() {
        let temp_dir = TempDir::new().unwrap();
        let (config, _inbox_path) = create_test_inbox(&temp_dir);

        let mut mock_emacs = MockEmacsClientTrait::new();
        mock_emacs
            .expect_eval()
            .withf(|elisp: &str| {
                elisp.contains("find-file-noselect") &&
                elisp.contains("Personal") &&
                elisp.contains("New test task")
            })
            .times(1)
            .returning(|_| Box::pin(async { Ok("nil".to_string()) }));

        let result = add_to_inbox(
            &config,
            &mock_emacs,
            "New test task",
            InboxSection::Personal,
            None,
            None,
            None,
            &[],
        ).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_add_to_inbox_with_scheduled() {
        let temp_dir = TempDir::new().unwrap();
        let (config, _) = create_test_inbox(&temp_dir);

        let mut mock_emacs = MockEmacsClientTrait::new();
        mock_emacs
            .expect_eval()
            .withf(|elisp: &str| {
                elisp.contains("SCHEDULED: <2026-03-10>")
            })
            .times(1)
            .returning(|_| Box::pin(async { Ok("nil".to_string()) }));

        let result = add_to_inbox(
            &config,
            &mock_emacs,
            "Scheduled task",
            InboxSection::Work,
            None,
            Some("2026-03-10"),
            None,
            &[],
        ).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_add_to_inbox_with_deadline() {
        let temp_dir = TempDir::new().unwrap();
        let (config, _) = create_test_inbox(&temp_dir);

        let mut mock_emacs = MockEmacsClientTrait::new();
        mock_emacs
            .expect_eval()
            .withf(|elisp: &str| {
                elisp.contains("DEADLINE: <2026-03-15>")
            })
            .times(1)
            .returning(|_| Box::pin(async { Ok("nil".to_string()) }));

        let result = add_to_inbox(
            &config,
            &mock_emacs,
            "Deadline task",
            InboxSection::Email,
            None,
            None,
            Some("2026-03-15"),
            &[],
        ).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_add_to_inbox_with_tags() {
        let temp_dir = TempDir::new().unwrap();
        let (config, _) = create_test_inbox(&temp_dir);

        let mut mock_emacs = MockEmacsClientTrait::new();
        mock_emacs
            .expect_eval()
            .withf(|elisp: &str| {
                elisp.contains(":urgent:important:")
            })
            .times(1)
            .returning(|_| Box::pin(async { Ok("nil".to_string()) }));

        let result = add_to_inbox(
            &config,
            &mock_emacs,
            "Tagged task",
            InboxSection::Personal,
            None,
            None,
            None,
            &["urgent".to_string(), "important".to_string()],
        ).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_add_to_inbox_with_body() {
        let temp_dir = TempDir::new().unwrap();
        let (config, _) = create_test_inbox(&temp_dir);

        let mut mock_emacs = MockEmacsClientTrait::new();
        mock_emacs
            .expect_eval()
            .withf(|elisp: &str| {
                elisp.contains("This is the task body")
            })
            .times(1)
            .returning(|_| Box::pin(async { Ok("nil".to_string()) }));

        let result = add_to_inbox(
            &config,
            &mock_emacs,
            "Task with body",
            InboxSection::Personal,
            Some("This is the task body"),
            None,
            None,
            &[],
        ).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_add_to_inbox_full_options() {
        let temp_dir = TempDir::new().unwrap();
        let (config, _) = create_test_inbox(&temp_dir);

        let mut mock_emacs = MockEmacsClientTrait::new();
        mock_emacs
            .expect_eval()
            .withf(|elisp: &str| {
                elisp.contains("Full options task") &&
                elisp.contains(":tag1:tag2:") &&
                elisp.contains("SCHEDULED: <2026-03-10>") &&
                elisp.contains("DEADLINE: <2026-03-20>") &&
                elisp.contains("Task body content")
            })
            .times(1)
            .returning(|_| Box::pin(async { Ok("nil".to_string()) }));

        let result = add_to_inbox(
            &config,
            &mock_emacs,
            "Full options task",
            InboxSection::Work,
            Some("Task body content"),
            Some("2026-03-10"),
            Some("2026-03-20"),
            &["tag1".to_string(), "tag2".to_string()],
        ).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_add_to_inbox_emacs_error() {
        let temp_dir = TempDir::new().unwrap();
        let (config, _) = create_test_inbox(&temp_dir);

        let mut mock_emacs = MockEmacsClientTrait::new();
        mock_emacs
            .expect_eval()
            .times(1)
            .returning(|_| Box::pin(async { Err(anyhow::anyhow!("Emacs eval failed")) }));

        let result = add_to_inbox(
            &config,
            &mock_emacs,
            "Test task",
            InboxSection::Personal,
            None,
            None,
            None,
            &[],
        ).await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Emacs eval failed"));
    }
}
