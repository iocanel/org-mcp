use super::Headline;
use anyhow::{Context, Result};
use lazy_static::lazy_static;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::path::Path;

lazy_static! {
    static ref TITLE_RE: Regex = Regex::new(r"#\+title:\s*(.+)").unwrap();
    static ref HEADLINE_START_RE: Regex = Regex::new(r"^\*+ ").unwrap();
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrgFile {
    pub path: String,
    pub title: Option<String>,
    pub headlines: Vec<Headline>,
}

impl OrgFile {
    pub fn parse(path: impl AsRef<Path>) -> Result<Self> {
        let path_ref = path.as_ref();
        let content = std::fs::read_to_string(path_ref)
            .with_context(|| format!("Failed to read file: {}", path_ref.display()))?;

        Self::parse_content(&content, path_ref.display().to_string())
    }

    pub fn parse_content(content: &str, path: String) -> Result<Self> {
        let title = TITLE_RE
            .captures(content)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().trim().to_string());

        let headlines = Self::parse_headlines(content);

        Ok(Self {
            path,
            title,
            headlines,
        })
    }

    fn parse_headlines(content: &str) -> Vec<Headline> {
        let mut headlines = Vec::new();
        let lines: Vec<&str> = content.lines().collect();
        let mut i = 0;

        while i < lines.len() {
            if HEADLINE_START_RE.is_match(lines[i]) {
                let line_number = i + 1;
                let start = i;
                i += 1;

                // Collect all lines until next headline of same or higher level
                while i < lines.len() {
                    if HEADLINE_START_RE.is_match(lines[i]) {
                        break;
                    }
                    i += 1;
                }

                let headline_text = lines[start..i].join("\n");
                if let Some(headline) = Headline::parse(&headline_text, line_number) {
                    headlines.push(headline);
                }
            } else {
                i += 1;
            }
        }

        headlines
    }

    pub fn find_headline_by_id(&self, id: &str) -> Option<&Headline> {
        self.headlines
            .iter()
            .find(|h| h.properties.id.as_ref().map_or(false, |i| i == id))
    }

    pub fn find_headlines_by_tag(&self, tag: &str) -> Vec<&Headline> {
        self.headlines.iter().filter(|h| h.has_tag(tag)).collect()
    }

    pub fn find_headlines_by_level(&self, level: usize) -> Vec<&Headline> {
        self.headlines
            .iter()
            .filter(|h| h.level == level)
            .collect()
    }

    pub fn find_section(&self, title: &str) -> Option<&Headline> {
        self.headlines
            .iter()
            .find(|h| h.title.eq_ignore_ascii_case(title))
    }

    pub fn get_children(&self, parent: &Headline) -> Vec<&Headline> {
        let parent_idx = self
            .headlines
            .iter()
            .position(|h| std::ptr::eq(h, parent))
            .unwrap_or(0);

        self.headlines
            .iter()
            .skip(parent_idx + 1)
            .take_while(|h| h.level > parent.level)
            .filter(|h| h.level == parent.level + 1)
            .collect()
    }

    pub fn get_all_descendants(&self, parent: &Headline) -> Vec<&Headline> {
        let parent_idx = self
            .headlines
            .iter()
            .position(|h| std::ptr::eq(h, parent))
            .unwrap_or(0);

        self.headlines
            .iter()
            .skip(parent_idx + 1)
            .take_while(|h| h.level > parent.level)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn create_test_file(content: &str) -> NamedTempFile {
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(content.as_bytes()).unwrap();
        file
    }

    #[test]
    fn test_parse_title() {
        let content = "#+title: Test File\n* Headline";
        let org = OrgFile::parse_content(content, "test.org".to_string()).unwrap();
        assert_eq!(org.title, Some("Test File".to_string()));
    }

    #[test]
    fn test_parse_no_title() {
        let content = "* Headline without title";
        let org = OrgFile::parse_content(content, "test.org".to_string()).unwrap();
        assert!(org.title.is_none());
    }

    #[test]
    fn test_parse_headlines() {
        let content = r#"#+title: Test
* First headline
** Second level
* Third headline"#;
        let org = OrgFile::parse_content(content, "test.org".to_string()).unwrap();
        assert_eq!(org.headlines.len(), 3);
        assert_eq!(org.headlines[0].title, "First headline");
        assert_eq!(org.headlines[0].level, 1);
        assert_eq!(org.headlines[1].title, "Second level");
        assert_eq!(org.headlines[1].level, 2);
        assert_eq!(org.headlines[2].title, "Third headline");
    }

    #[test]
    fn test_parse_from_file() {
        let content = r#"#+title: Test File
* TODO Task one
* DONE Task two"#;
        let file = create_test_file(content);
        let org = OrgFile::parse(file.path()).unwrap();
        assert_eq!(org.title, Some("Test File".to_string()));
        assert_eq!(org.headlines.len(), 2);
    }

    #[test]
    fn test_find_headline_by_id() {
        let content = r#"* Task
:PROPERTIES:
:ID:       unique-id
:END:"#;
        let org = OrgFile::parse_content(content, "test.org".to_string()).unwrap();
        let found = org.find_headline_by_id("unique-id");
        assert!(found.is_some());
        assert_eq!(found.unwrap().title, "Task");

        assert!(org.find_headline_by_id("nonexistent").is_none());
    }

    #[test]
    fn test_find_headlines_by_tag() {
        let content = r#"* Task one :work:
* Task two :personal:
* Task three :work:urgent:"#;
        let org = OrgFile::parse_content(content, "test.org".to_string()).unwrap();
        let work_tasks = org.find_headlines_by_tag("work");
        assert_eq!(work_tasks.len(), 2);
    }

    #[test]
    fn test_find_headlines_by_level() {
        let content = r#"* Level one
** Level two
*** Level three
* Another level one"#;
        let org = OrgFile::parse_content(content, "test.org".to_string()).unwrap();
        let level_one = org.find_headlines_by_level(1);
        assert_eq!(level_one.len(), 2);
    }

    #[test]
    fn test_find_section() {
        let content = r#"* Personal :personal:
** Task
* Work :work:
** Task"#;
        let org = OrgFile::parse_content(content, "test.org".to_string()).unwrap();
        let personal = org.find_section("Personal");
        assert!(personal.is_some());
        assert!(org.find_section("Nonexistent").is_none());
    }

    #[test]
    fn test_get_children() {
        let content = r#"* Parent
** Child 1
** Child 2
*** Grandchild
** Child 3
* Sibling"#;
        let org = OrgFile::parse_content(content, "test.org".to_string()).unwrap();
        let parent = &org.headlines[0];
        let children = org.get_children(parent);
        assert_eq!(children.len(), 3);
        assert_eq!(children[0].title, "Child 1");
        assert_eq!(children[1].title, "Child 2");
        assert_eq!(children[2].title, "Child 3");
    }

    #[test]
    fn test_get_all_descendants() {
        let content = r#"* Parent
** Child 1
*** Grandchild 1
** Child 2
* Sibling"#;
        let org = OrgFile::parse_content(content, "test.org".to_string()).unwrap();
        let parent = &org.headlines[0];
        let descendants = org.get_all_descendants(parent);
        assert_eq!(descendants.len(), 3);
    }

    #[test]
    fn test_parse_inbox_structure() {
        let content = r#"#+title: Inbox
* Personal :personal:
** TODO Personal task
SCHEDULED: <2026-03-06 Fri>
* Work :work:
** TODO Work task
SCHEDULED: <2026-03-06 Fri>
* Email :email:
** Follow Up
*** TODO Follow up task
** Read Later
*** TODO Read later task"#;
        let org = OrgFile::parse_content(content, "inbox.org".to_string()).unwrap();

        let personal = org.find_section("Personal").unwrap();
        assert!(personal.has_tag("personal"));

        let work = org.find_section("Work").unwrap();
        assert!(work.has_tag("work"));

        let email = org.find_section("Email").unwrap();
        assert!(email.has_tag("email"));
    }
}
