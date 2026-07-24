use crate::config::Config;
use crate::emacs::EmacsClientTrait;
use crate::models::Habit;
use crate::parser::OrgFile;
use anyhow::Result;
use chrono::{Local, NaiveDate};

/// Escape a value for interpolation inside an elisp double-quoted string.
fn elisp_str(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

pub fn get_habits(config: &Config) -> Result<Vec<Habit>> {
    let mut habits = Vec::new();

    for file_path in config.agenda_files() {
        if !file_path.exists() {
            continue;
        }

        let org = OrgFile::parse(&file_path)?;
        let file_str = file_path.display().to_string();

        for headline in &org.headlines {
            if let Some(habit) = Habit::from_headline(headline, &file_str) {
                habits.push(habit);
            }
        }
    }

    Ok(habits)
}

pub fn get_habits_due(config: &Config, date: NaiveDate) -> Result<Vec<Habit>> {
    let all_habits = get_habits(config)?;
    Ok(all_habits.into_iter().filter(|h| h.is_due(date)).collect())
}

pub fn get_habits_due_today(config: &Config) -> Result<Vec<Habit>> {
    let today = Local::now().date_naive();
    get_habits_due(config, today)
}

pub async fn create_habit<E: EmacsClientTrait>(
    emacs: &E,
    file_path: &str,
    title: &str,
    scheduled: &str,
    repeater: &str,
    tags: &[String],
) -> Result<()> {
    let mut headline = format!("* TODO {}", title);
    if !tags.is_empty() {
        headline.push_str(&format!(" :{}:", tags.join(":")));
    }

    // Org timestamps include the weekday name (e.g. <2026-07-24 Fri .+1w>); the
    // parser requires it, so derive it from the scheduled date.
    let scheduled_ts = match NaiveDate::parse_from_str(scheduled, "%Y-%m-%d") {
        Ok(d) => format!("{} {} {}", scheduled, d.format("%a"), repeater),
        Err(_) => format!("{} {}", scheduled, repeater),
    };

    // A habit is a TODO with a repeating SCHEDULED timestamp and :STYLE: habit.
    // Give it an :ID: (like org-roam nodes and the existing habits) so it can be
    // referenced/backlinked and found by id, not just title.
    let id = uuid::Uuid::new_v4().to_string();
    let content = format!(
        "{headline}\nSCHEDULED: <{scheduled_ts}>\n:PROPERTIES:\n:ID:       {id}\n:STYLE:    habit\n:END:"
    );

    let elisp = format!(
        r#"(with-current-buffer (find-file-noselect "{}")
  (goto-char (point-max))
  (insert "\n{}\n")
  (save-buffer))"#,
        elisp_str(file_path),
        elisp_str(&content)
    );

    emacs.eval(&elisp).await?;
    Ok(())
}

pub async fn delete_habit<E: EmacsClientTrait>(emacs: &E, habit: &Habit) -> Result<()> {
    let file_path = &habit.file_path;

    // Delete the whole subtree of the matching habit headline. regexp-quote the
    // title so special characters (+, *, ?, ., [ ...) are matched literally.
    let elisp = format!(
        r#"(with-current-buffer (find-file-noselect "{}")
  (goto-char (point-min))
  (when (re-search-forward (concat "^\\*+ TODO " (regexp-quote "{}")) nil t)
    (org-back-to-heading t)
    (org-cut-subtree))
  (save-buffer))"#,
        elisp_str(file_path),
        elisp_str(&habit.title)
    );

    emacs.eval(&elisp).await?;
    Ok(())
}

pub async fn mark_habit_done<E: EmacsClientTrait>(emacs: &E, habit: &Habit) -> Result<()> {
    let file_path = &habit.file_path;

    // Use emacsclient to mark the habit as done. regexp-quote the title so
    // special characters are matched literally.
    // org-habit will automatically reschedule it based on the repeater
    let elisp = format!(
        r#"(with-current-buffer (find-file-noselect "{}")
  (goto-char (point-min))
  (when (re-search-forward (concat "^\\*+ TODO " (regexp-quote "{}")) nil t)
    (org-todo 'done))
  (save-buffer))"#,
        elisp_str(file_path),
        elisp_str(&habit.title)
    );

    emacs.eval(&elisp).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::emacs::MockEmacsClientTrait;
    use tempfile::TempDir;

    fn create_test_habits(temp_dir: &TempDir) -> Config {
        let habits_path = temp_dir.path().join("habits.org");

        let habits_content = r#"#+title: Habits

* Habits
** TODO Log weight
SCHEDULED: <2026-03-06 Fri .+1d>
:PROPERTIES:
:ID:       habit-weight
:STYLE:    habit
:END:

** TODO Check emails
SCHEDULED: <2026-03-06 Fri .+1d>
:PROPERTIES:
:ID:       habit-email
:STYLE:    habit
:END:

** TODO Weekly review
SCHEDULED: <2026-03-07 Sat .+1w>
:PROPERTIES:
:ID:       habit-review
:STYLE:    habit
:END:

** TODO Exercise
SCHEDULED: <2026-03-05 Thu .+1d/3d>
:PROPERTIES:
:ID:       habit-exercise
:STYLE:    habit
:END:
"#;
        std::fs::write(&habits_path, habits_content).unwrap();

        Config {
            agenda: crate::config::AgendaConfig {
                files: vec![habits_path.display().to_string()],
            },
            inbox: crate::config::InboxConfig {
                file: String::new(),
                sections: vec![],
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

    #[test]
    fn test_get_habits() {
        let temp_dir = TempDir::new().unwrap();
        let config = create_test_habits(&temp_dir);

        let habits = get_habits(&config).unwrap();

        assert_eq!(habits.len(), 4);
        assert!(habits.iter().any(|h| h.title == "Log weight"));
        assert!(habits.iter().any(|h| h.title == "Check emails"));
        assert!(habits.iter().any(|h| h.title == "Weekly review"));
        assert!(habits.iter().any(|h| h.title == "Exercise"));
    }

    #[test]
    fn test_get_habits_due() {
        let temp_dir = TempDir::new().unwrap();
        let config = create_test_habits(&temp_dir);

        // March 6: Log weight, Check emails, Exercise should be due
        let march_6 = NaiveDate::from_ymd_opt(2026, 3, 6).unwrap();
        let due_habits = get_habits_due(&config, march_6).unwrap();

        assert_eq!(due_habits.len(), 3);
        assert!(due_habits.iter().any(|h| h.title == "Log weight"));
        assert!(due_habits.iter().any(|h| h.title == "Check emails"));
        assert!(due_habits.iter().any(|h| h.title == "Exercise"));

        // Weekly review is March 7, not due on March 6
        assert!(!due_habits.iter().any(|h| h.title == "Weekly review"));
    }

    #[test]
    fn test_get_habits_due_includes_weekly() {
        let temp_dir = TempDir::new().unwrap();
        let config = create_test_habits(&temp_dir);

        // March 7: All habits should be due
        let march_7 = NaiveDate::from_ymd_opt(2026, 3, 7).unwrap();
        let due_habits = get_habits_due(&config, march_7).unwrap();

        assert_eq!(due_habits.len(), 4);
        assert!(due_habits.iter().any(|h| h.title == "Weekly review"));
    }

    #[test]
    fn test_habit_repeater() {
        let temp_dir = TempDir::new().unwrap();
        let config = create_test_habits(&temp_dir);

        let habits = get_habits(&config).unwrap();

        let log_weight = habits.iter().find(|h| h.title == "Log weight").unwrap();
        assert_eq!(log_weight.repeater, Some(".+1d".to_string()));

        let weekly_review = habits.iter().find(|h| h.title == "Weekly review").unwrap();
        assert_eq!(weekly_review.repeater, Some(".+1w".to_string()));

        let exercise = habits.iter().find(|h| h.title == "Exercise").unwrap();
        assert_eq!(exercise.repeater, Some(".+1d/3d".to_string()));
    }

    #[test]
    fn test_empty_habits_file() {
        let config = Config {
            agenda: crate::config::AgendaConfig {
                files: vec!["/nonexistent/habits.org".to_string()],
            },
            inbox: crate::config::InboxConfig {
                file: String::new(),
                sections: vec![],
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

        let habits = get_habits(&config).unwrap();
        assert!(habits.is_empty());
    }

    #[tokio::test]
    async fn test_create_habit() {
        let mut mock_emacs = MockEmacsClientTrait::new();
        mock_emacs
            .expect_eval()
            .withf(|elisp: &str| {
                elisp.contains("find-file-noselect")
                    && elisp.contains("/path/to/habits.org")
                    && elisp.contains("TODO Weekly OSS contribution")
                    && elisp.contains(":oss:")
                    // day name derived from 2026-07-24 (a Friday) and repeater present
                    && elisp.contains("<2026-07-24 Fri .+1w>")
                    && elisp.contains(":STYLE:    habit")
            })
            .times(1)
            .returning(|_| Box::pin(async { Ok("nil".to_string()) }));

        let result = create_habit(
            &mock_emacs,
            "/path/to/habits.org",
            "Weekly OSS contribution",
            "2026-07-24",
            ".+1w",
            &["oss".to_string()],
        )
        .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_delete_habit_with_regex_special_title() {
        // Titles with regex-special chars (+, *, ?, .) must be regexp-quoted in
        // the elisp so the search matches literally.
        let habit = Habit {
            id: Some("habit-x".to_string()),
            title: "Review blog + talk idea buckets".to_string(),
            scheduled: Some(chrono::NaiveDate::from_ymd_opt(2026, 7, 24).unwrap()),
            repeater: Some(".+1w".to_string()),
            file_path: "/path/to/habits.org".to_string(),
            line_number: 10,
        };

        let mut mock_emacs = MockEmacsClientTrait::new();
        mock_emacs
            .expect_eval()
            .withf(|elisp: &str| {
                elisp.contains("regexp-quote")
                    && elisp.contains("Review blog + talk idea buckets")
                    && elisp.contains("org-cut-subtree")
            })
            .times(1)
            .returning(|_| Box::pin(async { Ok("nil".to_string()) }));

        assert!(delete_habit(&mock_emacs, &habit).await.is_ok());
    }

    #[tokio::test]
    async fn test_delete_habit() {
        let habit = Habit {
            id: Some("habit-blog".to_string()),
            title: "Monthly blog post".to_string(),
            scheduled: Some(chrono::NaiveDate::from_ymd_opt(2026, 7, 24).unwrap()),
            repeater: Some(".+1m".to_string()),
            file_path: "/path/to/habits.org".to_string(),
            line_number: 10,
        };

        let mut mock_emacs = MockEmacsClientTrait::new();
        mock_emacs
            .expect_eval()
            .withf(|elisp: &str| {
                elisp.contains("find-file-noselect")
                    && elisp.contains("Monthly blog post")
                    && elisp.contains("org-cut-subtree")
            })
            .times(1)
            .returning(|_| Box::pin(async { Ok("nil".to_string()) }));

        let result = delete_habit(&mock_emacs, &habit).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_mark_habit_done() {
        let habit = Habit {
            id: Some("habit-weight".to_string()),
            title: "Log weight".to_string(),
            scheduled: Some(chrono::NaiveDate::from_ymd_opt(2026, 3, 6).unwrap()),
            repeater: Some(".+1d".to_string()),
            file_path: "/path/to/habits.org".to_string(),
            line_number: 10,
        };

        let mut mock_emacs = MockEmacsClientTrait::new();
        mock_emacs
            .expect_eval()
            .withf(|elisp: &str| {
                elisp.contains("find-file-noselect") &&
                elisp.contains("/path/to/habits.org") &&
                elisp.contains("Log weight") &&
                elisp.contains("org-todo")
            })
            .times(1)
            .returning(|_| Box::pin(async { Ok("nil".to_string()) }));

        let result = mark_habit_done(&mock_emacs, &habit).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_mark_habit_done_with_special_chars() {
        let habit = Habit {
            id: Some("habit-test".to_string()),
            title: r#"Habit with "quotes" and \backslash"#.to_string(),
            scheduled: Some(chrono::NaiveDate::from_ymd_opt(2026, 3, 6).unwrap()),
            repeater: Some(".+1d".to_string()),
            file_path: "/path/to/habits.org".to_string(),
            line_number: 10,
        };

        let mut mock_emacs = MockEmacsClientTrait::new();
        mock_emacs
            .expect_eval()
            .withf(|elisp: &str| {
                // The function escapes \ to \\ and " to \"
                elisp.contains(r#"\"quotes\""#) &&
                elisp.contains(r"\\backslash")
            })
            .times(1)
            .returning(|_| Box::pin(async { Ok("nil".to_string()) }));

        let result = mark_habit_done(&mock_emacs, &habit).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_mark_habit_done_emacs_error() {
        let habit = Habit {
            id: Some("habit-test".to_string()),
            title: "Test habit".to_string(),
            scheduled: Some(chrono::NaiveDate::from_ymd_opt(2026, 3, 6).unwrap()),
            repeater: Some(".+1d".to_string()),
            file_path: "/path/to/habits.org".to_string(),
            line_number: 10,
        };

        let mut mock_emacs = MockEmacsClientTrait::new();
        mock_emacs
            .expect_eval()
            .times(1)
            .returning(|_| Box::pin(async { Err(anyhow::anyhow!("Emacs not running")) }));

        let result = mark_habit_done(&mock_emacs, &habit).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Emacs not running"));
    }
}
