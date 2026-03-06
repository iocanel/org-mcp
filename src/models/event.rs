use crate::parser::Headline;
use chrono::{NaiveDate, NaiveTime};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub id: Option<String>,
    pub title: String,
    pub date: NaiveDate,
    pub start_time: Option<NaiveTime>,
    pub end_time: Option<NaiveTime>,
    pub location: Option<String>,
    pub body: String,
    pub repeater: Option<String>,
    pub file_path: String,
    pub line_number: usize,
}

impl Event {
    pub fn from_headline(headline: &Headline, file_path: &str) -> Option<Self> {
        // Events have an active timestamp in the body (not SCHEDULED)
        // Look for timestamp in body or title
        let body_ts = crate::parser::OrgTimestamp::parse_active(&headline.body);

        let ts = body_ts?;

        Some(Self {
            id: headline.properties.id.clone(),
            title: headline.title.clone(),
            date: ts.date,
            start_time: ts.time,
            end_time: ts.end_time,
            location: headline.properties.location.clone(),
            body: headline.body.clone(),
            repeater: ts.repeater,
            file_path: file_path.to_string(),
            line_number: headline.line_number,
        })
    }

    pub fn is_on_date(&self, date: NaiveDate) -> bool {
        self.date == date
    }

    pub fn is_in_range(&self, start: NaiveDate, end: NaiveDate) -> bool {
        self.date >= start && self.date <= end
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::Properties;

    fn create_event_headline() -> Headline {
        Headline {
            level: 2,
            todo_state: None,
            priority: None,
            title: "Meeting with John".to_string(),
            tags: vec![],
            properties: Properties {
                id: Some("event-id".to_string()),
                location: Some("Coffee Shop".to_string()),
                ..Default::default()
            },
            scheduled: None,
            deadline: None,
            closed: None,
            body: "<2026-03-06 Fri 10:00-11:00>\nDiscuss project.".to_string(),
            line_number: 5,
        }
    }

    fn create_non_event_headline() -> Headline {
        Headline {
            level: 2,
            todo_state: None,
            priority: None,
            title: "Not an event".to_string(),
            tags: vec![],
            properties: Properties::default(),
            scheduled: None,
            deadline: None,
            closed: None,
            body: "No timestamp here".to_string(),
            line_number: 10,
        }
    }

    #[test]
    fn test_from_headline_event() {
        let headline = create_event_headline();
        let event = Event::from_headline(&headline, "calendar.org");

        assert!(event.is_some());
        let event = event.unwrap();
        assert_eq!(event.title, "Meeting with John");
        assert_eq!(event.date, NaiveDate::from_ymd_opt(2026, 3, 6).unwrap());
        assert_eq!(
            event.start_time,
            Some(NaiveTime::from_hms_opt(10, 0, 0).unwrap())
        );
        assert_eq!(
            event.end_time,
            Some(NaiveTime::from_hms_opt(11, 0, 0).unwrap())
        );
        assert_eq!(event.location, Some("Coffee Shop".to_string()));
    }

    #[test]
    fn test_from_headline_non_event() {
        let headline = create_non_event_headline();
        let event = Event::from_headline(&headline, "test.org");
        assert!(event.is_none());
    }

    #[test]
    fn test_is_on_date() {
        let headline = create_event_headline();
        let event = Event::from_headline(&headline, "calendar.org").unwrap();

        let march_6 = NaiveDate::from_ymd_opt(2026, 3, 6).unwrap();
        let march_7 = NaiveDate::from_ymd_opt(2026, 3, 7).unwrap();

        assert!(event.is_on_date(march_6));
        assert!(!event.is_on_date(march_7));
    }

    #[test]
    fn test_is_in_range() {
        let headline = create_event_headline();
        let event = Event::from_headline(&headline, "calendar.org").unwrap();

        let march_1 = NaiveDate::from_ymd_opt(2026, 3, 1).unwrap();
        let march_5 = NaiveDate::from_ymd_opt(2026, 3, 5).unwrap();
        let march_6 = NaiveDate::from_ymd_opt(2026, 3, 6).unwrap();
        let march_10 = NaiveDate::from_ymd_opt(2026, 3, 10).unwrap();

        assert!(event.is_in_range(march_1, march_10));
        assert!(event.is_in_range(march_6, march_6));
        assert!(!event.is_in_range(march_1, march_5));
    }
}
