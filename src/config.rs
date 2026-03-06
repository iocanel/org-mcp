use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub agenda: AgendaConfig,
    pub inbox: InboxConfig,
    pub refile: RefileConfig,
    pub emacs: EmacsConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgendaConfig {
    pub files: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InboxConfig {
    pub file: String,
    pub sections: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefileConfig {
    pub projects: String,
    pub areas: String,
    pub resources: String,
    pub archives: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmacsConfig {
    pub use_emacsclient: bool,
    pub socket_name: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            agenda: AgendaConfig {
                files: vec![
                    "~/Documents/org/roam/Inbox.org".to_string(),
                    "~/Documents/org/habits.org".to_string(),
                    "~/Documents/org/github.org".to_string(),
                    "~/Documents/org/calendars/personal.org".to_string(),
                    "~/Documents/org/calendars/work.org".to_string(),
                ],
            },
            inbox: InboxConfig {
                file: "~/Documents/org/roam/Inbox.org".to_string(),
                sections: vec![
                    "Personal".to_string(),
                    "Work".to_string(),
                    "Email".to_string(),
                ],
            },
            refile: RefileConfig {
                projects: "~/Documents/org/roam/Projects.org".to_string(),
                areas: "~/Documents/org/roam/Areas.org".to_string(),
                resources: "~/Documents/org/roam/Resources.org".to_string(),
                archives: "~/Documents/org/roam/Archives.org".to_string(),
            },
            emacs: EmacsConfig {
                use_emacsclient: true,
                socket_name: None,
            },
        }
    }
}

impl Config {
    pub fn load() -> Result<Self> {
        let config_path = Self::config_path()?;

        if config_path.exists() {
            let content = std::fs::read_to_string(&config_path)
                .with_context(|| format!("Failed to read config: {}", config_path.display()))?;
            let config: Config = toml::from_str(&content)
                .with_context(|| format!("Failed to parse config: {}", config_path.display()))?;
            Ok(config)
        } else {
            Ok(Self::default())
        }
    }

    pub fn save(&self) -> Result<()> {
        let config_path = Self::config_path()?;

        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let content = toml::to_string_pretty(self)?;
        std::fs::write(&config_path, content)?;

        Ok(())
    }

    fn config_path() -> Result<PathBuf> {
        let config_dir = directories::ProjectDirs::from("com", "iocanel", "org-mcp")
            .context("Could not determine config directory")?
            .config_dir()
            .to_path_buf();

        Ok(config_dir.join("config.toml"))
    }

    pub fn expand_path(path: &str) -> PathBuf {
        let expanded = shellexpand::tilde(path);
        PathBuf::from(expanded.as_ref())
    }

    pub fn agenda_files(&self) -> Vec<PathBuf> {
        self.agenda
            .files
            .iter()
            .map(|f| Self::expand_path(f))
            .collect()
    }

    pub fn inbox_file(&self) -> PathBuf {
        Self::expand_path(&self.inbox.file)
    }

    pub fn refile_targets(&self) -> Vec<(String, PathBuf)> {
        vec![
            ("Projects".to_string(), Self::expand_path(&self.refile.projects)),
            ("Areas".to_string(), Self::expand_path(&self.refile.areas)),
            (
                "Resources".to_string(),
                Self::expand_path(&self.refile.resources),
            ),
            ("Archives".to_string(), Self::expand_path(&self.refile.archives)),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert!(!config.agenda.files.is_empty());
        assert!(config.inbox.file.contains("Inbox.org"));
        assert_eq!(config.inbox.sections.len(), 3);
        assert!(config.emacs.use_emacsclient);
    }

    #[test]
    fn test_expand_path() {
        let path = Config::expand_path("~/Documents/test.org");
        assert!(!path.to_string_lossy().contains('~'));
        assert!(path.to_string_lossy().contains("Documents/test.org"));
    }

    #[test]
    fn test_agenda_files() {
        let config = Config::default();
        let files = config.agenda_files();
        assert!(!files.is_empty());
        for file in &files {
            assert!(!file.to_string_lossy().contains('~'));
        }
    }

    #[test]
    fn test_inbox_file() {
        let config = Config::default();
        let file = config.inbox_file();
        assert!(!file.to_string_lossy().contains('~'));
        assert!(file.to_string_lossy().contains("Inbox.org"));
    }

    #[test]
    fn test_refile_targets() {
        let config = Config::default();
        let targets = config.refile_targets();
        assert_eq!(targets.len(), 4);

        let names: Vec<&str> = targets.iter().map(|(n, _)| n.as_str()).collect();
        assert!(names.contains(&"Projects"));
        assert!(names.contains(&"Areas"));
        assert!(names.contains(&"Resources"));
        assert!(names.contains(&"Archives"));
    }

    #[test]
    fn test_serialize_deserialize() {
        let config = Config::default();
        let toml_str = toml::to_string_pretty(&config).unwrap();
        let parsed: Config = toml::from_str(&toml_str).unwrap();

        assert_eq!(config.agenda.files, parsed.agenda.files);
        assert_eq!(config.inbox.file, parsed.inbox.file);
    }
}
