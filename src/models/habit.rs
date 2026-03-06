use crate::parser::Headline;
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Habit {
    pub id: Option<String>,
    pub title: String,
    pub scheduled: Option<NaiveDate>,
    pub repeater: Option<String>,
    pub file_path: String,
    pub line_number: usize,
}

impl Habit {
    pub fn from_headline(headline: &Headline, file_path: &str) -> Option<Self> {
        if !headline.is_habit() {
            return None;
        }

        Some(Self {
            id: headline.properties.id.clone(),
            title: headline.title.clone(),
            scheduled: headline.scheduled.as_ref().map(|ts| ts.date),
            repeater: headline.scheduled.as_ref().and_then(|ts| ts.repeater.clone()),
            file_path: file_path.to_string(),
            line_number: headline.line_number,
        })
    }

    pub fn is_due(&self, date: NaiveDate) -> bool {
        self.scheduled.map_or(false, |d| d <= date)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::{OrgTimestamp, Properties, TimestampType, TodoState};

    fn create_habit_headline() -> Headline {
        Headline {
            level: 2,
            todo_state: Some(TodoState::Todo),
            priority: None,
            title: "Log weight".to_string(),
            tags: vec![],
            properties: Properties {
                id: Some("habit-id".to_string()),
                style: Some("habit".to_string()),
                ..Default::default()
            },
            scheduled: Some(OrgTimestamp {
                timestamp_type: TimestampType::Scheduled,
                date: NaiveDate::from_ymd_opt(2026, 3, 6).unwrap(),
                time: None,
                end_time: None,
                repeater: Some(".+1d".to_string()),
            }),
            deadline: None,
            closed: None,
            body: String::new(),
            line_number: 5,
        }
    }

    fn create_non_habit_headline() -> Headline {
        Headline {
            level: 2,
            todo_state: Some(TodoState::Todo),
            priority: None,
            title: "Regular task".to_string(),
            tags: vec![],
            properties: Properties::default(),
            scheduled: None,
            deadline: None,
            closed: None,
            body: String::new(),
            line_number: 10,
        }
    }

    #[test]
    fn test_from_headline_habit() {
        let headline = create_habit_headline();
        let habit = Habit::from_headline(&headline, "habits.org");

        assert!(habit.is_some());
        let habit = habit.unwrap();
        assert_eq!(habit.id, Some("habit-id".to_string()));
        assert_eq!(habit.title, "Log weight");
        assert_eq!(habit.repeater, Some(".+1d".to_string()));
    }

    #[test]
    fn test_from_headline_non_habit() {
        let headline = create_non_habit_headline();
        let habit = Habit::from_headline(&headline, "test.org");
        assert!(habit.is_none());
    }

    #[test]
    fn test_is_due() {
        let headline = create_habit_headline();
        let habit = Habit::from_headline(&headline, "habits.org").unwrap();

        let march_5 = NaiveDate::from_ymd_opt(2026, 3, 5).unwrap();
        let march_6 = NaiveDate::from_ymd_opt(2026, 3, 6).unwrap();
        let march_7 = NaiveDate::from_ymd_opt(2026, 3, 7).unwrap();

        assert!(!habit.is_due(march_5));
        assert!(habit.is_due(march_6));
        assert!(habit.is_due(march_7));
    }
}
