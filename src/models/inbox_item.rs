use crate::parser::{Headline, TodoState};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InboxSection {
    Personal,
    Work,
    Email,
}

impl InboxSection {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "personal" => Some(Self::Personal),
            "work" => Some(Self::Work),
            "email" => Some(Self::Email),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Personal => "Personal",
            Self::Work => "Work",
            Self::Email => "Email",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InboxItem {
    pub id: Option<String>,
    pub title: String,
    pub section: InboxSection,
    pub state: Option<TodoState>,
    pub priority: Option<char>,
    pub tags: Vec<String>,
    pub scheduled: Option<NaiveDate>,
    pub deadline: Option<NaiveDate>,
    pub body: String,
    pub line_number: usize,
}

impl InboxItem {
    pub fn from_headline(headline: &Headline, section: InboxSection) -> Self {
        Self {
            id: headline.properties.id.clone(),
            title: headline.title.clone(),
            section,
            state: headline.todo_state,
            priority: headline.priority,
            tags: headline.tags.clone(),
            scheduled: headline.scheduled.as_ref().map(|ts| ts.date),
            deadline: headline.deadline.as_ref().map(|ts| ts.date),
            body: headline.body.clone(),
            line_number: headline.line_number,
        }
    }

    pub fn is_done(&self) -> bool {
        self.state.map_or(false, |s| s.is_done())
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
            priority: Some('B'),
            title: "Inbox item".to_string(),
            tags: vec!["urgent".to_string()],
            properties: Properties {
                id: Some("inbox-item-id".to_string()),
                ..Default::default()
            },
            scheduled: Some(OrgTimestamp {
                timestamp_type: TimestampType::Scheduled,
                date: NaiveDate::from_ymd_opt(2026, 3, 6).unwrap(),
                time: None,
                end_time: None,
                repeater: None,
            }),
            deadline: None,
            closed: None,
            body: "Item body".to_string(),
            line_number: 15,
        }
    }

    #[test]
    fn test_inbox_section_from_str() {
        assert_eq!(
            InboxSection::from_str("personal"),
            Some(InboxSection::Personal)
        );
        assert_eq!(
            InboxSection::from_str("PERSONAL"),
            Some(InboxSection::Personal)
        );
        assert_eq!(InboxSection::from_str("work"), Some(InboxSection::Work));
        assert_eq!(InboxSection::from_str("email"), Some(InboxSection::Email));
        assert_eq!(InboxSection::from_str("invalid"), None);
    }

    #[test]
    fn test_inbox_section_as_str() {
        assert_eq!(InboxSection::Personal.as_str(), "Personal");
        assert_eq!(InboxSection::Work.as_str(), "Work");
        assert_eq!(InboxSection::Email.as_str(), "Email");
    }

    #[test]
    fn test_from_headline() {
        let headline = create_test_headline();
        let item = InboxItem::from_headline(&headline, InboxSection::Work);

        assert_eq!(item.id, Some("inbox-item-id".to_string()));
        assert_eq!(item.title, "Inbox item");
        assert_eq!(item.section, InboxSection::Work);
        assert_eq!(item.state, Some(TodoState::Todo));
        assert_eq!(item.priority, Some('B'));
        assert_eq!(item.tags, vec!["urgent"]);
        assert_eq!(
            item.scheduled,
            Some(NaiveDate::from_ymd_opt(2026, 3, 6).unwrap())
        );
    }

    #[test]
    fn test_is_done() {
        let headline = create_test_headline();
        let mut item = InboxItem::from_headline(&headline, InboxSection::Personal);

        assert!(!item.is_done());

        item.state = Some(TodoState::Done);
        assert!(item.is_done());
    }
}
