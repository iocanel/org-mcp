use crate::config::Config;
use crate::emacs::EmacsClientTrait;
use crate::models::Habit;
use crate::parser::OrgFile;
use anyhow::Result;
use chrono::{Local, NaiveDate};

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

pub async fn mark_habit_done<E: EmacsClientTrait>(emacs: &E, habit: &Habit) -> Result<()> {
    let file_path = &habit.file_path;

    // Use emacsclient to mark the habit as done
    // org-habit will automatically reschedule it based on the repeater
    let elisp = format!(
        r#"(with-current-buffer (find-file-noselect "{}")
  (goto-char (point-min))
  (when (re-search-forward "^\\*+ TODO {}" nil t)
    (org-todo 'done))
  (save-buffer))"#,
        file_path,
        habit.title.replace('\\', "\\\\").replace('"', "\\\"")
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
