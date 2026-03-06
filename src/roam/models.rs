use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrgRoamNode {
    pub id: String,
    pub file: String,
    pub level: i32,
    pub pos: i32,
    pub todo: Option<String>,
    pub priority: Option<String>,
    pub scheduled: Option<String>,
    pub deadline: Option<String>,
    pub title: Option<String>,
    pub properties: Option<String>,
    pub olp: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub aliases: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrgRoamLink {
    pub pos: i32,
    pub source: String,
    pub dest: String,
    pub link_type: String,
    pub properties: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrgRoamFile {
    pub file: String,
    pub title: Option<String>,
    pub hash: String,
    pub atime: i64,
    pub mtime: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseStats {
    pub nodes: i64,
    pub files: i64,
    pub links: i64,
    pub unique_tags: i64,
    pub aliases: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_org_roam_node_default_tags() {
        let node = OrgRoamNode {
            id: "test-id".to_string(),
            file: "/path/to/file.org".to_string(),
            level: 0,
            pos: 1,
            todo: None,
            priority: None,
            scheduled: None,
            deadline: None,
            title: Some("Test Node".to_string()),
            properties: None,
            olp: None,
            tags: vec![],
            aliases: vec![],
        };

        assert!(node.tags.is_empty());
        assert!(node.aliases.is_empty());
    }

    #[test]
    fn test_org_roam_link() {
        let link = OrgRoamLink {
            pos: 100,
            source: "source-id".to_string(),
            dest: "dest-id".to_string(),
            link_type: "id".to_string(),
            properties: None,
        };

        assert_eq!(link.source, "source-id");
        assert_eq!(link.dest, "dest-id");
    }

    #[test]
    fn test_database_stats() {
        let stats = DatabaseStats {
            nodes: 100,
            files: 50,
            links: 200,
            unique_tags: 30,
            aliases: 25,
        };

        assert_eq!(stats.nodes, 100);
        assert_eq!(stats.files, 50);
    }
}
