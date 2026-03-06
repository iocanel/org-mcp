use anyhow::{Context, Result};
use std::process::Stdio;
use thiserror::Error;
use tokio::process::Command;

#[cfg(test)]
use mockall::automock;

#[derive(Error, Debug)]
pub enum EmacsError {
    #[error("Emacs daemon not running")]
    DaemonNotRunning,
    #[error("Elisp evaluation failed: {0}")]
    EvalFailed(String),
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

#[cfg_attr(test, automock)]
pub trait EmacsClientTrait: Send + Sync {
    fn eval(&self, elisp: &str) -> impl std::future::Future<Output = Result<String>> + Send;
    fn eval_silent(&self, elisp: &str) -> impl std::future::Future<Output = Result<()>> + Send;
}

#[derive(Debug, Clone)]
pub struct EmacsClient {
    socket_name: Option<String>,
}

impl EmacsClient {
    pub fn new() -> Self {
        Self { socket_name: None }
    }

    pub fn with_socket(socket_name: impl Into<String>) -> Self {
        Self {
            socket_name: Some(socket_name.into()),
        }
    }

    async fn run_emacsclient(&self, args: &[&str]) -> Result<String> {
        let mut cmd = Command::new("emacsclient");

        if let Some(ref socket) = self.socket_name {
            cmd.arg("--socket-name").arg(socket);
        }

        cmd.args(args);
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        let output = cmd
            .output()
            .await
            .context("Failed to execute emacsclient")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("connect") || stderr.contains("socket") {
                return Err(EmacsError::DaemonNotRunning.into());
            }
            return Err(EmacsError::EvalFailed(stderr.to_string()).into());
        }

        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(stdout)
    }
}

impl Default for EmacsClient {
    fn default() -> Self {
        Self::new()
    }
}

impl EmacsClientTrait for EmacsClient {
    async fn eval(&self, elisp: &str) -> Result<String> {
        self.run_emacsclient(&["--eval", elisp]).await
    }

    async fn eval_silent(&self, elisp: &str) -> Result<()> {
        self.run_emacsclient(&["--eval", elisp]).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_emacs_client_new() {
        let client = EmacsClient::new();
        assert!(client.socket_name.is_none());
    }

    #[test]
    fn test_emacs_client_with_socket() {
        let client = EmacsClient::with_socket("server");
        assert_eq!(client.socket_name, Some("server".to_string()));
    }

    #[test]
    fn test_emacs_client_default() {
        let client = EmacsClient::default();
        assert!(client.socket_name.is_none());
    }

    #[tokio::test]
    #[ignore = "Requires Emacs daemon to be running"]
    async fn test_eval_simple_expression() {
        let client = EmacsClient::new();
        let result = client.eval("(+ 1 2)").await;
        match result {
            Ok(value) => assert_eq!(value, "3"),
            Err(e) => {
                let err_str = e.to_string();
                assert!(
                    err_str.contains("daemon")
                        || err_str.contains("connect")
                        || err_str.contains("socket")
                        || err_str.contains("emacsclient"),
                    "Unexpected error: {}",
                    err_str
                );
            }
        }
    }

    #[tokio::test]
    #[ignore = "Requires Emacs daemon to be running"]
    async fn test_eval_get_agenda_files() {
        let client = EmacsClient::new();
        let result = client.eval("(prin1-to-string org-agenda-files)").await;
        match result {
            Ok(value) => {
                assert!(value.starts_with('(') || value.starts_with("nil"));
            }
            Err(e) => {
                let err_str = e.to_string();
                assert!(
                    err_str.contains("daemon")
                        || err_str.contains("connect")
                        || err_str.contains("socket")
                        || err_str.contains("emacsclient"),
                    "Unexpected error: {}",
                    err_str
                );
            }
        }
    }

    #[test]
    fn test_emacs_error_display() {
        let err = EmacsError::DaemonNotRunning;
        assert_eq!(err.to_string(), "Emacs daemon not running");

        let err = EmacsError::EvalFailed("test error".to_string());
        assert_eq!(err.to_string(), "Elisp evaluation failed: test error");
    }
}
