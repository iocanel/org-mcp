use lazy_static::lazy_static;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

lazy_static! {
    static ref PROPERTY_DRAWER_RE: Regex =
        Regex::new(r"(?s):PROPERTIES:\n(.*?):END:").unwrap();
    static ref PROPERTY_LINE_RE: Regex =
        Regex::new(r":([A-Za-z_][A-Za-z0-9_-]*):\s*(.*)").unwrap();
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Properties {
    pub id: Option<String>,
    pub style: Option<String>,
    pub location: Option<String>,
    pub other: HashMap<String, String>,
}

impl Properties {
    pub fn parse(text: &str) -> Self {
        let mut props = Self::default();

        if let Some(caps) = PROPERTY_DRAWER_RE.captures(text) {
            let drawer_content = caps.get(1).map_or("", |m| m.as_str());

            for line in drawer_content.lines() {
                if let Some(prop_caps) = PROPERTY_LINE_RE.captures(line) {
                    let key = prop_caps.get(1).map_or("", |m| m.as_str());
                    let value = prop_caps.get(2).map_or("", |m| m.as_str()).trim();

                    match key.to_uppercase().as_str() {
                        "ID" => props.id = Some(value.to_string()),
                        "STYLE" => props.style = Some(value.to_string()),
                        "LOCATION" => props.location = Some(value.to_string()),
                        _ => {
                            props.other.insert(key.to_string(), value.to_string());
                        }
                    }
                }
            }
        }

        props
    }

    pub fn is_habit(&self) -> bool {
        self.style.as_ref().map_or(false, |s| s == "habit")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_properties_with_id() {
        let text = r#":PROPERTIES:
:ID:       test-id-123
:END:"#;
        let props = Properties::parse(text);
        assert_eq!(props.id, Some("test-id-123".to_string()));
    }

    #[test]
    fn test_parse_properties_with_style() {
        let text = r#":PROPERTIES:
:ID:       habit-id
:STYLE:    habit
:END:"#;
        let props = Properties::parse(text);
        assert_eq!(props.id, Some("habit-id".to_string()));
        assert_eq!(props.style, Some("habit".to_string()));
        assert!(props.is_habit());
    }

    #[test]
    fn test_parse_properties_with_location() {
        let text = r#":PROPERTIES:
:ID:       event-id
:LOCATION: Coffee Shop
:END:"#;
        let props = Properties::parse(text);
        assert_eq!(props.location, Some("Coffee Shop".to_string()));
    }

    #[test]
    fn test_parse_properties_with_custom() {
        let text = r#":PROPERTIES:
:ID:       custom-id
:CUSTOM_PROP: custom value
:END:"#;
        let props = Properties::parse(text);
        assert_eq!(
            props.other.get("CUSTOM_PROP"),
            Some(&"custom value".to_string())
        );
    }

    #[test]
    fn test_parse_no_properties() {
        let text = "Some text without properties";
        let props = Properties::parse(text);
        assert!(props.id.is_none());
        assert!(props.style.is_none());
        assert!(props.other.is_empty());
    }

    #[test]
    fn test_is_habit_false() {
        let props = Properties::default();
        assert!(!props.is_habit());

        let props = Properties {
            style: Some("other".to_string()),
            ..Default::default()
        };
        assert!(!props.is_habit());
    }

    #[test]
    fn test_properties_default() {
        let props = Properties::default();
        assert!(props.id.is_none());
        assert!(props.style.is_none());
        assert!(props.location.is_none());
        assert!(props.other.is_empty());
    }
}
