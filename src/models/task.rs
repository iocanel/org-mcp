use crate::parser::{Headline, TodoState};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: Option<String>,
    pub title: String,
    pub state: Option<TodoState>,
    pub priority: Option<char>,
    pub tags: Vec<String>,
    pub scheduled: Option<NaiveDate>,
    pub deadline: Option<NaiveDate>,
    pub body: String,
    pub file_path: String,
    pub line_number: usize,
}

impl Task {
    pub fn from_headline(headline: &Headline, file_path: &str) -> Self {
        Self {
            id: headline.properties.id.clone(),
            title: headline.title.clone(),
            state: headline.todo_state,
            priority: headline.priority,
            tags: headline.tags.clone(),
            scheduled: headline.scheduled.as_ref().map(|ts| ts.date),
            deadline: headline.deadline.as_ref().map(|ts| ts.date),
            body: headline.body.clone(),
            file_path: file_path.to_string(),
            line_number: headline.line_number,
        }
    }

    pub fn is_done(&self) -> bool {
        self.state.map_or(false, |s| s.is_done())
    }

    pub fn is_scheduled_for(&self, date: NaiveDate) -> bool {
        self.scheduled.map_or(false, |d| d == date)
    }

    pub fn is_scheduled_on_or_before(&self, date: NaiveDate) -> bool {
        self.scheduled.map_or(false, |d| d <= date)
    }

    pub fn has_deadline_on_or_before(&self, date: NaiveDate) -> bool {
        self.deadline.map_or(false, |d| d <= date)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::{OrgTimestamp, Properties, TimestampType};

    fn create_test_headline() -> Headline {
        Headline {
            level: 2,
            todo_state: Some(TodoState::Todo),
            priority: Some('A'),
            title: "Test task".to_string(),
            tags: vec!["work".to_string()],
            properties: Properties {
                id: Some("task-id".to_string()),
                ..Default::default()
            },
            scheduled: Some(OrgTimestamp {
                timestamp_type: TimestampType::Scheduled,
                date: NaiveDate::from_ymd_opt(2026, 3, 6).unwrap(),
                time: None,
                end_time: None,
                repeater: None,
            }),
            deadline: Some(OrgTimestamp {
                timestamp_type: TimestampType::Deadline,
                date: NaiveDate::from_ymd_opt(2026, 3, 10).unwrap(),
                time: None,
                end_time: None,
                repeater: None,
            }),
            closed: None,
            body: "Task body".to_string(),
            line_number: 10,
        }
    }

    #[test]
    fn test_from_headline() {
        let headline = create_test_headline();
        let task = Task::from_headline(&headline, "/path/to/file.org");

        assert_eq!(task.id, Some("task-id".to_string()));
        assert_eq!(task.title, "Test task");
        assert_eq!(task.state, Some(TodoState::Todo));
        assert_eq!(task.priority, Some('A'));
        assert_eq!(task.tags, vec!["work"]);
        assert_eq!(
            task.scheduled,
            Some(NaiveDate::from_ymd_opt(2026, 3, 6).unwrap())
        );
        assert_eq!(
            task.deadline,
            Some(NaiveDate::from_ymd_opt(2026, 3, 10).unwrap())
        );
        assert_eq!(task.file_path, "/path/to/file.org");
        assert_eq!(task.line_number, 10);
    }

    #[test]
    fn test_is_done() {
        let mut task = Task::from_headline(&create_test_headline(), "test.org");
        assert!(!task.is_done());

        task.state = Some(TodoState::Done);
        assert!(task.is_done());

        task.state = Some(TodoState::Cancelled);
        assert!(task.is_done());

        task.state = None;
        assert!(!task.is_done());
    }

    #[test]
    fn test_is_scheduled_for() {
        let task = Task::from_headline(&create_test_headline(), "test.org");
        let march_6 = NaiveDate::from_ymd_opt(2026, 3, 6).unwrap();
        let march_7 = NaiveDate::from_ymd_opt(2026, 3, 7).unwrap();

        assert!(task.is_scheduled_for(march_6));
        assert!(!task.is_scheduled_for(march_7));
    }

    #[test]
    fn test_is_scheduled_on_or_before() {
        let task = Task::from_headline(&create_test_headline(), "test.org");
        let march_5 = NaiveDate::from_ymd_opt(2026, 3, 5).unwrap();
        let march_6 = NaiveDate::from_ymd_opt(2026, 3, 6).unwrap();
        let march_7 = NaiveDate::from_ymd_opt(2026, 3, 7).unwrap();

        assert!(!task.is_scheduled_on_or_before(march_5));
        assert!(task.is_scheduled_on_or_before(march_6));
        assert!(task.is_scheduled_on_or_before(march_7));
    }

    #[test]
    fn test_has_deadline_on_or_before() {
        let task = Task::from_headline(&create_test_headline(), "test.org");
        let march_9 = NaiveDate::from_ymd_opt(2026, 3, 9).unwrap();
        let march_10 = NaiveDate::from_ymd_opt(2026, 3, 10).unwrap();
        let march_11 = NaiveDate::from_ymd_opt(2026, 3, 11).unwrap();

        assert!(!task.has_deadline_on_or_before(march_9));
        assert!(task.has_deadline_on_or_before(march_10));
        assert!(task.has_deadline_on_or_before(march_11));
    }
}
