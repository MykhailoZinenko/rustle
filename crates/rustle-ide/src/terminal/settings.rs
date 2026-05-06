#[allow(dead_code, unused_imports, deprecated, clippy::all)]
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct BackendSettings {
    pub shell: String,
    pub args: Vec<String>,
    pub working_directory: Option<PathBuf>,
}

impl BackendSettings {
    pub fn default_shell() -> String {
        #[cfg(target_os = "windows")]
        {
            std::env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".to_string())
        }

        #[cfg(target_os = "macos")]
        {
            "/bin/zsh".to_string()
        }

        #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
        {
            "/bin/bash".to_string()
        }
    }
}

impl Default for BackendSettings {
    fn default() -> Self {
        Self {
            shell: Self::default_shell(),
            args: vec![],
            working_directory: None,
        }
    }
}
