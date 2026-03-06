use crate::config::Config;
use crate::models::{Event, Habit, Task};
use crate::parser::OrgFile;
use anyhow::Result;
use chrono::{Local, NaiveDate};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgendaDay {
    pub date: NaiveDate,
    pub tasks: Vec<Task>,
    pub habits: Vec<Habit>,
    pub events: Vec<Event>,
}

impl AgendaDay {
    pub fn new(date: NaiveDate) -> Self {
        Self {
            date,
            tasks: Vec::new(),
            habits: Vec::new(),
            events: Vec::new(),
        }
    }
}

pub fn get_agenda_today(config: &Config) -> Result<AgendaDay> {
    let today = Local::now().date_naive();
    get_agenda_for_date(config, today)
}

pub fn get_agenda_upcoming(config: &Config, days: usize) -> Result<Vec<AgendaDay>> {
    let today = Local::now().date_naive();
    let mut agenda_days = Vec::new();

    for i in 0..days {
        let date = today + chrono::Duration::days(i as i64);
        let day = get_agenda_for_date(config, date)?;
        agenda_days.push(day);
    }

    Ok(agenda_days)
}

pub fn get_agenda_for_date(config: &Config, date: NaiveDate) -> Result<AgendaDay> {
    let mut agenda = AgendaDay::new(date);

    for file_path in config.agenda_files() {
        if !file_path.exists() {
            continue;
        }

        let org = OrgFile::parse(&file_path)?;
        let file_str = file_path.display().to_string();

        for headline in &org.headlines {
            // Check if it's a habit
            if headline.is_habit() {
                if let Some(habit) = Habit::from_headline(headline, &file_str) {
                    if habit.is_due(date) {
                        agenda.habits.push(habit);
                    }
                }
                continue;
            }

            // Check if it's an event (has active timestamp in body)
            if let Some(event) = Event::from_headline(headline, &file_str) {
                if event.is_on_date(date) {
                    agenda.events.push(event);
                }
                continue;
            }

            // Check if it's a task scheduled for this date
            if headline.todo_state.is_some() && !headline.is_done() {
                let task = Task::from_headline(headline, &file_str);

                // Include if scheduled for today or earlier (overdue)
                // Or if deadline is today or earlier
                if task.is_scheduled_on_or_before(date) || task.has_deadline_on_or_before(date) {
                    agenda.tasks.push(task);
                }
            }
        }
    }

    Ok(agenda)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    use tempfile::TempDir;

    fn create_test_config(temp_dir: &TempDir) -> Config {
        let inbox_path = temp_dir.path().join("inbox.org");
        let habits_path = temp_dir.path().join("habits.org");
        let calendar_path = temp_dir.path().join("calendar.org");

        // Create inbox file
        let inbox_content = r#"#+title: Inbox
* Personal :personal:
** TODO Personal task
SCHEDULED: <2026-03-06 Fri>
:PROPERTIES:
:ID:       personal-task-id
:END:
* Work :work:
** TODO Work task
SCHEDULED: <2026-03-06 Fri>
DEADLINE: <2026-03-10 Tue>
:PROPERTIES:
:ID:       work-task-id
:END:
** TODO Overdue task
SCHEDULED: <2026-03-05 Thu>
:PROPERTIES:
:ID:       overdue-task-id
:END:
** DONE Completed task
:PROPERTIES:
:ID:       completed-task-id
:END:"#;
        std::fs::write(&inbox_path, inbox_content).unwrap();

        // Create habits file
        let habits_content = r#"#+title: Habits
* Habits
** TODO Log weight
SCHEDULED: <2026-03-06 Fri .+1d>
:PROPERTIES:
:ID:       habit-weight-id
:STYLE:    habit
:END:
** TODO Check emails
SCHEDULED: <2026-03-06 Fri .+1d>
:PROPERTIES:
:ID:       habit-email-id
:STYLE:    habit
:END:"#;
        std::fs::write(&habits_path, habits_content).unwrap();

        // Create calendar file
        let calendar_content = r#"#+title: Calendar
* Events
** Meeting with John
<2026-03-06 Fri 10:00-11:00>
:PROPERTIES:
:ID:       meeting-id
:LOCATION: Coffee Shop
:END:
** Future meeting
<2026-03-10 Tue 14:00-15:00>
:PROPERTIES:
:ID:       future-meeting-id
:END:"#;
        std::fs::write(&calendar_path, calendar_content).unwrap();

        Config {
            agenda: crate::config::AgendaConfig {
                files: vec![
                    inbox_path.display().to_string(),
                    habits_path.display().to_string(),
                    calendar_path.display().to_string(),
                ],
            },
            inbox: crate::config::InboxConfig {
                file: inbox_path.display().to_string(),
                sections: vec!["Personal".to_string(), "Work".to_string()],
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
    fn test_get_agenda_for_date() {
        let temp_dir = TempDir::new().unwrap();
        let config = create_test_config(&temp_dir);

        let date = NaiveDate::from_ymd_opt(2026, 3, 6).unwrap();
        let agenda = get_agenda_for_date(&config, date).unwrap();

        // Should have tasks scheduled for March 6
        assert!(!agenda.tasks.is_empty());
        assert!(agenda.tasks.iter().any(|t| t.title == "Personal task"));
        assert!(agenda.tasks.iter().any(|t| t.title == "Work task"));

        // Should include overdue task (scheduled March 5)
        assert!(agenda.tasks.iter().any(|t| t.title == "Overdue task"));

        // Should NOT include completed task
        assert!(!agenda.tasks.iter().any(|t| t.title == "Completed task"));

        // Should have habits
        assert_eq!(agenda.habits.len(), 2);
        assert!(agenda.habits.iter().any(|h| h.title == "Log weight"));

        // Should have the meeting event
        assert_eq!(agenda.events.len(), 1);
        assert_eq!(agenda.events[0].title, "Meeting with John");
    }

    #[test]
    fn test_get_agenda_upcoming() {
        let temp_dir = TempDir::new().unwrap();
        let config = create_test_config(&temp_dir);

        // We need to test with dates relative to our fixtures
        // Since we use Local::now() in the actual function, let's test the
        // underlying get_agenda_for_date with specific dates

        let march_6 = NaiveDate::from_ymd_opt(2026, 3, 6).unwrap();
        let march_10 = NaiveDate::from_ymd_opt(2026, 3, 10).unwrap();

        let agenda_6 = get_agenda_for_date(&config, march_6).unwrap();
        let agenda_10 = get_agenda_for_date(&config, march_10).unwrap();

        // March 6 should have Meeting with John
        assert!(agenda_6.events.iter().any(|e| e.title == "Meeting with John"));

        // March 10 should have Future meeting
        assert!(agenda_10
            .events
            .iter()
            .any(|e| e.title == "Future meeting"));

        // March 10 should include Work task (deadline is March 10)
        assert!(agenda_10.tasks.iter().any(|t| t.title == "Work task"));
    }

    #[test]
    fn test_agenda_day_new() {
        let date = NaiveDate::from_ymd_opt(2026, 3, 6).unwrap();
        let agenda = AgendaDay::new(date);

        assert_eq!(agenda.date, date);
        assert!(agenda.tasks.is_empty());
        assert!(agenda.habits.is_empty());
        assert!(agenda.events.is_empty());
    }

    #[test]
    fn test_missing_file_is_skipped() {
        let config = Config {
            agenda: crate::config::AgendaConfig {
                files: vec!["/nonexistent/file.org".to_string()],
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

        let date = NaiveDate::from_ymd_opt(2026, 3, 6).unwrap();
        let result = get_agenda_for_date(&config, date);

        // Should not error, just return empty agenda
        assert!(result.is_ok());
        let agenda = result.unwrap();
        assert!(agenda.tasks.is_empty());
    }
}
