use crate::roam::models::{DatabaseStats, OrgRoamFile, OrgRoamLink, OrgRoamNode};
use anyhow::{Context, Result};
use rusqlite::{Connection, OpenFlags};
use std::path::{Path, PathBuf};

pub struct OrgRoamDatabase {
    conn: Connection,
    db_path: PathBuf,
}

impl OrgRoamDatabase {
    pub fn open(db_path: impl AsRef<Path>) -> Result<Self> {
        let db_path = db_path.as_ref().to_path_buf();

        if !db_path.exists() {
            anyhow::bail!("Org-roam database not found at: {}", db_path.display());
        }

        let conn = Connection::open_with_flags(&db_path, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .with_context(|| format!("Failed to open org-roam database: {}", db_path.display()))?;

        Ok(Self { conn, db_path })
    }

    pub fn find_database() -> Result<PathBuf> {
        let home = dirs::home_dir().context("Could not determine home directory")?;

        let possible_paths = [
            home.join(".emacs.d/org-roam.db"),
            home.join("org-roam.db"),
            home.join(".config/emacs/org-roam.db"),
            home.join("Documents/org-roam/org-roam.db"),
            home.join("Documents/org/roam/org-roam.db"),
        ];

        for path in &possible_paths {
            if path.exists() {
                return Ok(path.clone());
            }
        }

        anyhow::bail!(
            "Could not find org-roam database. Searched: {:?}",
            possible_paths
        )
    }

    fn clean_string(value: Option<&str>) -> Option<String> {
        value.map(|s| s.trim_matches('"').to_string())
    }

    fn normalize_id(id: &str) -> String {
        if id.starts_with('"') && id.ends_with('"') {
            id.to_string()
        } else {
            format!("\"{}\"", id)
        }
    }

    pub fn get_all_nodes(&self, limit: Option<usize>) -> Result<Vec<OrgRoamNode>> {
        let query = if let Some(lim) = limit {
            format!(
                "SELECT id, file, level, pos, todo, priority, scheduled, deadline, \
                 title, properties, olp FROM nodes ORDER BY title LIMIT {}",
                lim
            )
        } else {
            "SELECT id, file, level, pos, todo, priority, scheduled, deadline, \
             title, properties, olp FROM nodes ORDER BY title"
                .to_string()
        };

        let mut stmt = self.conn.prepare(&query)?;
        let nodes = stmt
            .query_map([], |row| {
                Ok(OrgRoamNode {
                    id: Self::clean_string(row.get::<_, Option<String>>(0)?.as_deref())
                        .unwrap_or_default(),
                    file: Self::clean_string(row.get::<_, Option<String>>(1)?.as_deref())
                        .unwrap_or_default(),
                    level: row.get(2)?,
                    pos: row.get(3)?,
                    todo: row.get(4)?,
                    priority: row.get(5)?,
                    scheduled: row.get(6)?,
                    deadline: row.get(7)?,
                    title: Self::clean_string(row.get::<_, Option<String>>(8)?.as_deref()),
                    properties: row.get(9)?,
                    olp: row.get(10)?,
                    tags: vec![],
                    aliases: vec![],
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(nodes)
    }

    pub fn get_node_by_id(&self, node_id: &str) -> Result<Option<OrgRoamNode>> {
        let search_id = Self::normalize_id(node_id);

        let mut stmt = self.conn.prepare(
            "SELECT id, file, level, pos, todo, priority, scheduled, deadline, \
             title, properties, olp FROM nodes WHERE id = ?",
        )?;

        let mut rows = stmt.query([&search_id])?;

        if let Some(row) = rows.next()? {
            let mut node = OrgRoamNode {
                id: Self::clean_string(row.get::<_, Option<String>>(0)?.as_deref())
                    .unwrap_or_default(),
                file: Self::clean_string(row.get::<_, Option<String>>(1)?.as_deref())
                    .unwrap_or_default(),
                level: row.get(2)?,
                pos: row.get(3)?,
                todo: row.get(4)?,
                priority: row.get(5)?,
                scheduled: row.get(6)?,
                deadline: row.get(7)?,
                title: Self::clean_string(row.get::<_, Option<String>>(8)?.as_deref()),
                properties: row.get(9)?,
                olp: row.get(10)?,
                tags: vec![],
                aliases: vec![],
            };

            node.tags = self.get_node_tags(&node.id)?;
            node.aliases = self.get_node_aliases(&node.id)?;

            Ok(Some(node))
        } else {
            Ok(None)
        }
    }

    pub fn search_nodes(&self, query: &str, limit: Option<usize>) -> Result<Vec<OrgRoamNode>> {
        let search_pattern = format!("%{}%", query);

        let sql = if let Some(lim) = limit {
            format!(
                "SELECT DISTINCT n.id, n.file, n.level, n.pos, n.todo, n.priority, \
                 n.scheduled, n.deadline, n.title, n.properties, n.olp \
                 FROM nodes n \
                 LEFT JOIN aliases a ON n.id = a.node_id \
                 LEFT JOIN tags t ON n.id = t.node_id \
                 WHERE n.title LIKE ? OR a.alias LIKE ? OR t.tag LIKE ? \
                 ORDER BY n.title LIMIT {}",
                lim
            )
        } else {
            "SELECT DISTINCT n.id, n.file, n.level, n.pos, n.todo, n.priority, \
             n.scheduled, n.deadline, n.title, n.properties, n.olp \
             FROM nodes n \
             LEFT JOIN aliases a ON n.id = a.node_id \
             LEFT JOIN tags t ON n.id = t.node_id \
             WHERE n.title LIKE ? OR a.alias LIKE ? OR t.tag LIKE ? \
             ORDER BY n.title"
                .to_string()
        };

        let mut stmt = self.conn.prepare(&sql)?;
        let nodes = stmt
            .query_map([&search_pattern, &search_pattern, &search_pattern], |row| {
                Ok(OrgRoamNode {
                    id: Self::clean_string(row.get::<_, Option<String>>(0)?.as_deref())
                        .unwrap_or_default(),
                    file: Self::clean_string(row.get::<_, Option<String>>(1)?.as_deref())
                        .unwrap_or_default(),
                    level: row.get(2)?,
                    pos: row.get(3)?,
                    todo: row.get(4)?,
                    priority: row.get(5)?,
                    scheduled: row.get(6)?,
                    deadline: row.get(7)?,
                    title: Self::clean_string(row.get::<_, Option<String>>(8)?.as_deref()),
                    properties: row.get(9)?,
                    olp: row.get(10)?,
                    tags: vec![],
                    aliases: vec![],
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(nodes)
    }

    pub fn get_backlinks(&self, node_id: &str) -> Result<Vec<OrgRoamLink>> {
        let search_id = Self::normalize_id(node_id);

        let mut stmt = self
            .conn
            .prepare("SELECT pos, source, dest, type, properties FROM links WHERE dest = ?")?;

        let links = stmt
            .query_map([&search_id], |row| {
                Ok(OrgRoamLink {
                    pos: row.get(0)?,
                    source: row.get(1)?,
                    dest: row.get(2)?,
                    link_type: row.get(3)?,
                    properties: row.get(4)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(links)
    }

    pub fn get_forward_links(&self, node_id: &str) -> Result<Vec<OrgRoamLink>> {
        let search_id = Self::normalize_id(node_id);

        let mut stmt = self
            .conn
            .prepare("SELECT pos, source, dest, type, properties FROM links WHERE source = ?")?;

        let links = stmt
            .query_map([&search_id], |row| {
                Ok(OrgRoamLink {
                    pos: row.get(0)?,
                    source: row.get(1)?,
                    dest: row.get(2)?,
                    link_type: row.get(3)?,
                    properties: row.get(4)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(links)
    }

    pub fn get_node_tags(&self, node_id: &str) -> Result<Vec<String>> {
        let search_id = Self::normalize_id(node_id);

        let mut stmt = self
            .conn
            .prepare("SELECT tag FROM tags WHERE node_id = ?")?;

        let tags = stmt
            .query_map([&search_id], |row| row.get(0))?
            .collect::<Result<Vec<String>, _>>()?;

        Ok(tags)
    }

    pub fn get_node_aliases(&self, node_id: &str) -> Result<Vec<String>> {
        let search_id = Self::normalize_id(node_id);

        let mut stmt = self
            .conn
            .prepare("SELECT alias FROM aliases WHERE node_id = ?")?;

        let aliases = stmt
            .query_map([&search_id], |row| row.get(0))?
            .collect::<Result<Vec<String>, _>>()?;

        Ok(aliases)
    }

    pub fn get_all_files(&self) -> Result<Vec<OrgRoamFile>> {
        let mut stmt = self
            .conn
            .prepare("SELECT file, title, hash, atime, mtime FROM files ORDER BY file")?;

        let files = stmt
            .query_map([], |row| {
                Ok(OrgRoamFile {
                    file: row.get(0)?,
                    title: row.get(1)?,
                    hash: row.get(2)?,
                    atime: row.get(3)?,
                    mtime: row.get(4)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(files)
    }

    pub fn get_database_stats(&self) -> Result<DatabaseStats> {
        let nodes: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM nodes", [], |row| row.get(0))?;

        let files: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM files", [], |row| row.get(0))?;

        let links: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM links", [], |row| row.get(0))?;

        let unique_tags: i64 = self
            .conn
            .query_row("SELECT COUNT(DISTINCT tag) FROM tags", [], |row| row.get(0))?;

        let aliases: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM aliases", [], |row| row.get(0))?;

        Ok(DatabaseStats {
            nodes,
            files,
            links,
            unique_tags,
            aliases,
        })
    }

    pub fn refresh(&mut self) -> Result<()> {
        self.conn = Connection::open_with_flags(&self.db_path, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .with_context(|| {
                format!(
                    "Failed to refresh org-roam database: {}",
                    self.db_path.display()
                )
            })?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_database(temp_dir: &TempDir) -> PathBuf {
        let db_path = temp_dir.path().join("org-roam.db");

        let conn = Connection::open(&db_path).unwrap();

        conn.execute_batch(
            r#"
            CREATE TABLE nodes (
                id TEXT PRIMARY KEY,
                file TEXT,
                level INTEGER,
                pos INTEGER,
                todo TEXT,
                priority TEXT,
                scheduled TEXT,
                deadline TEXT,
                title TEXT,
                properties TEXT,
                olp TEXT
            );

            CREATE TABLE links (
                pos INTEGER,
                source TEXT,
                dest TEXT,
                type TEXT,
                properties TEXT
            );

            CREATE TABLE tags (
                node_id TEXT,
                tag TEXT
            );

            CREATE TABLE aliases (
                node_id TEXT,
                alias TEXT
            );

            CREATE TABLE files (
                file TEXT PRIMARY KEY,
                title TEXT,
                hash TEXT,
                atime INTEGER,
                mtime INTEGER
            );

            INSERT INTO nodes VALUES (
                '"test-node-1"',
                '"/home/user/org/roam/test.org"',
                0,
                1,
                NULL,
                NULL,
                NULL,
                NULL,
                '"Test Node One"',
                NULL,
                NULL
            );

            INSERT INTO nodes VALUES (
                '"test-node-2"',
                '"/home/user/org/roam/another.org"',
                0,
                1,
                'TODO',
                'A',
                NULL,
                NULL,
                '"Test Node Two"',
                NULL,
                NULL
            );

            INSERT INTO tags VALUES ('"test-node-1"', 'tag1');
            INSERT INTO tags VALUES ('"test-node-1"', 'tag2');

            INSERT INTO aliases VALUES ('"test-node-1"', 'alias1');

            INSERT INTO links VALUES (100, '"test-node-1"', '"test-node-2"', 'id', NULL);

            INSERT INTO files VALUES (
                '/home/user/org/roam/test.org',
                'Test File',
                'abc123',
                1609459200,
                1609459200
            );
            "#,
        )
        .unwrap();

        db_path
    }

    #[test]
    fn test_open_database() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = create_test_database(&temp_dir);

        let db = OrgRoamDatabase::open(&db_path);
        assert!(db.is_ok());
    }

    #[test]
    fn test_open_nonexistent_database() {
        let result = OrgRoamDatabase::open("/nonexistent/path/org-roam.db");
        assert!(result.is_err());
    }

    #[test]
    fn test_get_all_nodes() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = create_test_database(&temp_dir);
        let db = OrgRoamDatabase::open(&db_path).unwrap();

        let nodes = db.get_all_nodes(None).unwrap();
        assert_eq!(nodes.len(), 2);
    }

    #[test]
    fn test_get_all_nodes_with_limit() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = create_test_database(&temp_dir);
        let db = OrgRoamDatabase::open(&db_path).unwrap();

        let nodes = db.get_all_nodes(Some(1)).unwrap();
        assert_eq!(nodes.len(), 1);
    }

    #[test]
    fn test_get_node_by_id() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = create_test_database(&temp_dir);
        let db = OrgRoamDatabase::open(&db_path).unwrap();

        let node = db.get_node_by_id("test-node-1").unwrap();
        assert!(node.is_some());

        let node = node.unwrap();
        assert_eq!(node.id, "test-node-1");
        assert_eq!(node.title, Some("Test Node One".to_string()));
        assert_eq!(node.tags.len(), 2);
        assert!(node.tags.contains(&"tag1".to_string()));
        assert_eq!(node.aliases.len(), 1);
    }

    #[test]
    fn test_get_node_by_id_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = create_test_database(&temp_dir);
        let db = OrgRoamDatabase::open(&db_path).unwrap();

        let node = db.get_node_by_id("nonexistent").unwrap();
        assert!(node.is_none());
    }

    #[test]
    fn test_search_nodes() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = create_test_database(&temp_dir);
        let db = OrgRoamDatabase::open(&db_path).unwrap();

        let nodes = db.search_nodes("Test", None).unwrap();
        assert_eq!(nodes.len(), 2);

        let nodes = db.search_nodes("One", None).unwrap();
        assert_eq!(nodes.len(), 1);
    }

    #[test]
    fn test_search_nodes_by_tag() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = create_test_database(&temp_dir);
        let db = OrgRoamDatabase::open(&db_path).unwrap();

        let nodes = db.search_nodes("tag1", None).unwrap();
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].id, "test-node-1");
    }

    #[test]
    fn test_get_backlinks() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = create_test_database(&temp_dir);
        let db = OrgRoamDatabase::open(&db_path).unwrap();

        let links = db.get_backlinks("test-node-2").unwrap();
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].source, "\"test-node-1\"");
    }

    #[test]
    fn test_get_forward_links() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = create_test_database(&temp_dir);
        let db = OrgRoamDatabase::open(&db_path).unwrap();

        let links = db.get_forward_links("test-node-1").unwrap();
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].dest, "\"test-node-2\"");
    }

    #[test]
    fn test_get_node_tags() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = create_test_database(&temp_dir);
        let db = OrgRoamDatabase::open(&db_path).unwrap();

        let tags = db.get_node_tags("test-node-1").unwrap();
        assert_eq!(tags.len(), 2);
        assert!(tags.contains(&"tag1".to_string()));
        assert!(tags.contains(&"tag2".to_string()));
    }

    #[test]
    fn test_get_node_aliases() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = create_test_database(&temp_dir);
        let db = OrgRoamDatabase::open(&db_path).unwrap();

        let aliases = db.get_node_aliases("test-node-1").unwrap();
        assert_eq!(aliases.len(), 1);
        assert_eq!(aliases[0], "alias1");
    }

    #[test]
    fn test_get_all_files() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = create_test_database(&temp_dir);
        let db = OrgRoamDatabase::open(&db_path).unwrap();

        let files = db.get_all_files().unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].title, Some("Test File".to_string()));
    }

    #[test]
    fn test_get_database_stats() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = create_test_database(&temp_dir);
        let db = OrgRoamDatabase::open(&db_path).unwrap();

        let stats = db.get_database_stats().unwrap();
        assert_eq!(stats.nodes, 2);
        assert_eq!(stats.files, 1);
        assert_eq!(stats.links, 1);
        assert_eq!(stats.unique_tags, 2);
        assert_eq!(stats.aliases, 1);
    }
}
