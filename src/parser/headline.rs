use super::{OrgTimestamp, Properties};
use lazy_static::lazy_static;
use regex::Regex;
use serde::{Deserialize, Serialize};

lazy_static! {
    // Headline pattern: stars, optional TODO state, optional priority, title, optional tags
    static ref HEADLINE_RE: Regex = Regex::new(
        r"^(\*+)\s+(?:(TODO|DONE|NEXT|WAITING|CANCELLED)\s+)?(?:\[#([ABC])\]\s+)?(.+?)(?:\s+:([^:]+(?::[^:]+)*):)?$"
    ).unwrap();
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TodoState {
    Todo,
    Done,
    Next,
    Waiting,
    Cancelled,
}

impl TodoState {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_uppercase().as_str() {
            "TODO" => Some(Self::Todo),
            "DONE" => Some(Self::Done),
            "NEXT" => Some(Self::Next),
            "WAITING" => Some(Self::Waiting),
            "CANCELLED" => Some(Self::Cancelled),
            _ => None,
        }
    }

    pub fn is_done(&self) -> bool {
        matches!(self, Self::Done | Self::Cancelled)
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Todo => "TODO",
            Self::Done => "DONE",
            Self::Next => "NEXT",
            Self::Waiting => "WAITING",
            Self::Cancelled => "CANCELLED",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Headline {
    pub level: usize,
    pub todo_state: Option<TodoState>,
    pub priority: Option<char>,
    pub title: String,
    pub tags: Vec<String>,
    pub properties: Properties,
    pub scheduled: Option<OrgTimestamp>,
    pub deadline: Option<OrgTimestamp>,
    pub closed: Option<OrgTimestamp>,
    pub body: String,
    pub line_number: usize,
}

impl Headline {
    pub fn parse(text: &str, line_number: usize) -> Option<Self> {
        let mut lines = text.lines();
        let first_line = lines.next()?;

        let caps = HEADLINE_RE.captures(first_line)?;

        let level = caps.get(1)?.as_str().len();
        let todo_state = caps.get(2).and_then(|m| TodoState::from_str(m.as_str()));
        let priority = caps.get(3).and_then(|m| m.as_str().chars().next());
        let title = caps.get(4)?.as_str().trim().to_string();
        let tags: Vec<String> = caps
            .get(5)
            .map(|m| m.as_str().split(':').map(|s| s.to_string()).collect())
            .unwrap_or_default();

        let rest: String = lines.collect::<Vec<_>>().join("\n");

        let properties = Properties::parse(&rest);
        let scheduled = OrgTimestamp::parse_scheduled(&rest);
        let deadline = OrgTimestamp::parse_deadline(&rest);
        let closed = OrgTimestamp::parse_closed(&rest);

        // Extract body (content after properties and timestamps)
        let body = Self::extract_body(&rest);

        Some(Self {
            level,
            todo_state,
            priority,
            title,
            tags,
            properties,
            scheduled,
            deadline,
            closed,
            body,
            line_number,
        })
    }

    fn extract_body(text: &str) -> String {
        let mut in_drawer = false;
        let mut body_lines = Vec::new();

        for line in text.lines() {
            let trimmed = line.trim();

            if trimmed.starts_with(":PROPERTIES:") || trimmed.starts_with(":LOGBOOK:") {
                in_drawer = true;
                continue;
            }

            if trimmed == ":END:" {
                in_drawer = false;
                continue;
            }

            if in_drawer {
                continue;
            }

            // Skip SCHEDULED/DEADLINE/CLOSED lines
            if trimmed.starts_with("SCHEDULED:")
                || trimmed.starts_with("DEADLINE:")
                || trimmed.starts_with("CLOSED:")
            {
                continue;
            }

            body_lines.push(line);
        }

        body_lines.join("\n").trim().to_string()
    }

    pub fn is_done(&self) -> bool {
        self.todo_state.map_or(false, |s| s.is_done())
    }

    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t.eq_ignore_ascii_case(tag))
    }

    pub fn is_habit(&self) -> bool {
        self.properties.is_habit()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_headline() {
        let text = "* Simple headline";
        let headline = Headline::parse(text, 1).unwrap();
        assert_eq!(headline.level, 1);
        assert!(headline.todo_state.is_none());
        assert_eq!(headline.title, "Simple headline");
        assert!(headline.tags.is_empty());
    }

    #[test]
    fn test_parse_todo_headline() {
        let text = "** TODO Task to do";
        let headline = Headline::parse(text, 1).unwrap();
        assert_eq!(headline.level, 2);
        assert_eq!(headline.todo_state, Some(TodoState::Todo));
        assert_eq!(headline.title, "Task to do");
    }

    #[test]
    fn test_parse_done_headline() {
        let text = "* DONE Completed task";
        let headline = Headline::parse(text, 1).unwrap();
        assert_eq!(headline.todo_state, Some(TodoState::Done));
        assert!(headline.is_done());
    }

    #[test]
    fn test_parse_headline_with_priority() {
        let text = "* TODO [#A] High priority task";
        let headline = Headline::parse(text, 1).unwrap();
        assert_eq!(headline.priority, Some('A'));
        assert_eq!(headline.title, "High priority task");
    }

    #[test]
    fn test_parse_headline_with_tags() {
        let text = "* TODO Task with tags :work:urgent:";
        let headline = Headline::parse(text, 1).unwrap();
        assert_eq!(headline.tags, vec!["work", "urgent"]);
        assert!(headline.has_tag("work"));
        assert!(headline.has_tag("WORK")); // case insensitive
        assert!(!headline.has_tag("personal"));
    }

    #[test]
    fn test_parse_headline_with_properties() {
        let text = r#"* TODO Task
:PROPERTIES:
:ID:       task-123
:END:"#;
        let headline = Headline::parse(text, 1).unwrap();
        assert_eq!(headline.properties.id, Some("task-123".to_string()));
    }

    #[test]
    fn test_parse_headline_with_scheduled() {
        let text = r#"* TODO Scheduled task
SCHEDULED: <2026-03-06 Fri>"#;
        let headline = Headline::parse(text, 1).unwrap();
        assert!(headline.scheduled.is_some());
        assert_eq!(
            headline.scheduled.unwrap().date,
            chrono::NaiveDate::from_ymd_opt(2026, 3, 6).unwrap()
        );
    }

    #[test]
    fn test_parse_headline_with_deadline() {
        let text = r#"* TODO Task with deadline
DEADLINE: <2026-03-10 Tue>"#;
        let headline = Headline::parse(text, 1).unwrap();
        assert!(headline.deadline.is_some());
    }

    #[test]
    fn test_parse_headline_with_body() {
        let text = r#"* TODO Task with body
:PROPERTIES:
:ID:       body-task
:END:
SCHEDULED: <2026-03-06 Fri>
This is the body content.
Multiple lines of text."#;
        let headline = Headline::parse(text, 1).unwrap();
        assert!(headline.body.contains("This is the body content"));
        assert!(headline.body.contains("Multiple lines"));
    }

    #[test]
    fn test_parse_habit_headline() {
        let text = r#"** TODO Log weight
SCHEDULED: <2026-03-06 Fri .+1d>
:PROPERTIES:
:ID:       habit-id
:STYLE:    habit
:END:"#;
        let headline = Headline::parse(text, 1).unwrap();
        assert!(headline.is_habit());
        assert!(headline.scheduled.unwrap().is_repeating());
    }

    #[test]
    fn test_todo_state_from_str() {
        assert_eq!(TodoState::from_str("TODO"), Some(TodoState::Todo));
        assert_eq!(TodoState::from_str("todo"), Some(TodoState::Todo));
        assert_eq!(TodoState::from_str("DONE"), Some(TodoState::Done));
        assert_eq!(TodoState::from_str("NEXT"), Some(TodoState::Next));
        assert_eq!(TodoState::from_str("WAITING"), Some(TodoState::Waiting));
        assert_eq!(TodoState::from_str("CANCELLED"), Some(TodoState::Cancelled));
        assert_eq!(TodoState::from_str("invalid"), None);
    }

    #[test]
    fn test_todo_state_is_done() {
        assert!(!TodoState::Todo.is_done());
        assert!(TodoState::Done.is_done());
        assert!(!TodoState::Next.is_done());
        assert!(!TodoState::Waiting.is_done());
        assert!(TodoState::Cancelled.is_done());
    }

    #[test]
    fn test_todo_state_as_str() {
        assert_eq!(TodoState::Todo.as_str(), "TODO");
        assert_eq!(TodoState::Done.as_str(), "DONE");
        assert_eq!(TodoState::Next.as_str(), "NEXT");
        assert_eq!(TodoState::Waiting.as_str(), "WAITING");
        assert_eq!(TodoState::Cancelled.as_str(), "CANCELLED");
    }

    #[test]
    fn test_parse_invalid_headline() {
        assert!(Headline::parse("Not a headline", 1).is_none());
        assert!(Headline::parse("", 1).is_none());
    }
}
