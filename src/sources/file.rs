//! File-based configuration source

use crate::error::{ConfigError, Result};
use crate::sources::ConfigSource;
use crate::value::ConfigValue;
use async_trait::async_trait;
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::fs;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

/// Configuration source that loads from files
pub struct FileSource {
    files: Vec<FileConfig>,
    name: String,
}

#[derive(Debug, Clone)]
struct FileConfig {
    path: PathBuf,
    namespace: Option<String>,
    required: bool,
}

impl FileSource {
    /// Create a new file source
    pub fn new() -> Self {
        Self {
            files: Vec::new(),
            name: "file".to_string(),
        }
    }

    /// Add a configuration file
    pub fn add_file<P: AsRef<Path>>(mut self, path: P, namespace: Option<&str>) -> Self {
        self.files.push(FileConfig {
            path: path.as_ref().to_path_buf(),
            namespace: namespace.map(|s| s.to_string()),
            required: true,
        });
        self
    }

    /// Add an optional configuration file (won't fail if missing)
    pub fn add_optional_file<P: AsRef<Path>>(mut self, path: P, namespace: Option<&str>) -> Self {
        self.files.push(FileConfig {
            path: path.as_ref().to_path_buf(),
            namespace: namespace.map(|s| s.to_string()),
            required: false,
        });
        self
    }

    /// Set the name of this source
    pub fn with_name<S: Into<String>>(mut self, name: S) -> Self {
        self.name = name.into();
        self
    }

    /// Load configuration from a single file
    async fn load_file(&self, file_config: &FileConfig) -> Result<Option<ConfigValue>> {
        let path = &file_config.path;

        // Check if file exists
        if !path.exists() {
            if file_config.required {
                return Err(ConfigError::FileError(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!("Required configuration file not found: {}", path.display()),
                )));
            } else {
                debug!("Optional configuration file not found: {}", path.display());
                return Ok(None);
            }
        }

        info!("Loading configuration from: {}", path.display());

        // Read file content
        let content = fs::read_to_string(path).await?;
        if content.trim().is_empty() {
            warn!("Configuration file is empty: {}", path.display());
            return Ok(Some(ConfigValue::Object(HashMap::new())));
        }

        // Parse based on file extension
        let config_value = match path.extension().and_then(|ext| ext.to_str()) {
            Some("json") => {
                let json_value: serde_json::Value = serde_json::from_str(&content)?;
                ConfigValue::from(json_value)
            }
            Some("yaml") | Some("yml") => {
                let yaml_value: serde_yaml::Value = serde_yaml::from_str(&content)?;
                // Convert yaml to json first, then to ConfigValue
                let json_value = serde_json::to_value(yaml_value)?;
                ConfigValue::from(json_value)
            }
            Some("toml") => {
                let toml_value: toml::Value = toml::from_str(&content)?;
                // Convert toml to json first, then to ConfigValue
                let json_value = serde_json::to_value(toml_value)?;
                ConfigValue::from(json_value)
            }
            Some(ext) => {
                return Err(ConfigError::Other(format!(
                    "Unsupported file format: {ext}"
                )));
            }
            None => {
                return Err(ConfigError::Other(format!(
                    "Cannot determine file format for: {}",
                    path.display()
                )));
            }
        };

        // Wrap in namespace if specified
        let final_value = if let Some(namespace) = &file_config.namespace {
            let mut wrapper = HashMap::new();
            wrapper.insert(namespace.clone(), config_value);
            ConfigValue::Object(wrapper)
        } else {
            config_value
        };

        Ok(Some(final_value))
    }
}

impl Default for FileSource {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ConfigSource for FileSource {
    async fn load(&self) -> Result<ConfigValue> {
        let mut merged_config = ConfigValue::Object(HashMap::new());

        for file_config in &self.files {
            if let Some(config) = self.load_file(file_config).await? {
                merged_config.merge(config);
            }
        }

        Ok(merged_config)
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn supports_watching(&self) -> bool {
        true
    }

    async fn start_watching(&self) -> Result<tokio::sync::mpsc::Receiver<ConfigValue>> {
        let (tx, rx) = mpsc::channel(100);

        // Clone the file configurations for the watcher
        let files = self.files.clone();
        let source_name = self.name.clone();

        tokio::spawn(async move {
            let (watcher_tx, mut watcher_rx) = mpsc::channel(100);

            // Create the file watcher
            let mut watcher = match RecommendedWatcher::new(
                move |res: std::result::Result<Event, notify::Error>| {
                    let tx = watcher_tx.clone();
                    tokio::spawn(async move {
                        if let Err(e) = tx.send(res).await {
                            error!("Failed to send file event: {}", e);
                        }
                    });
                },
                notify::Config::default(),
            ) {
                Ok(w) => w,
                Err(e) => {
                    error!("Failed to create file watcher: {}", e);
                    return;
                }
            };

            // Watch all configured files
            for file_config in &files {
                if let Some(parent) = file_config.path.parent() {
                    if let Err(e) = watcher.watch(parent, RecursiveMode::NonRecursive) {
                        error!("Failed to watch directory {}: {}", parent.display(), e);
                    }
                } else if let Err(e) = watcher.watch(&file_config.path, RecursiveMode::NonRecursive)
                {
                    error!("Failed to watch file {}: {}", file_config.path.display(), e);
                }
            }

            info!("File watcher started for source: {}", source_name);

            // Process file events
            while let Some(event_result) = watcher_rx.recv().await {
                match event_result {
                    Ok(event) => {
                        // Check if the event is for one of our watched files
                        let should_reload = match event.kind {
                            EventKind::Modify(_) | EventKind::Create(_) => {
                                event.paths.iter().any(|path| {
                                    files.iter().any(|file_config| {
                                        path.file_name() == file_config.path.file_name()
                                    })
                                })
                            }
                            _ => false,
                        };

                        if should_reload {
                            info!("Configuration file changed, reloading...");

                            // Reload the configuration
                            let file_source = FileSource {
                                files: files.clone(),
                                name: source_name.clone(),
                            };

                            match file_source.load().await {
                                Ok(config) => {
                                    if let Err(e) = tx.send(config).await {
                                        error!("Failed to send config update: {}", e);
                                        break;
                                    }
                                }
                                Err(e) => {
                                    error!("Failed to reload configuration: {}", e);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        error!("File watcher error: {}", e);
                    }
                }
            }

            info!("File watcher stopped for source: {}", source_name);
        });

        Ok(rx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn test_load_json_file() {
        let mut temp_file = NamedTempFile::with_suffix(".json").unwrap();
        let content = r#"{"key": "value", "number": 42}"#;
        temp_file.write_all(content.as_bytes()).unwrap();

        let source = FileSource::new().add_file(temp_file.path(), None);

        let config = source.load().await.unwrap();
        assert_eq!(
            config.get_path("key").unwrap().as_string().unwrap(),
            "value"
        );
        assert_eq!(config.get_path("number").unwrap().as_integer().unwrap(), 42);
    }

    #[tokio::test]
    async fn test_load_yaml_file() {
        let mut temp_file = NamedTempFile::with_suffix(".yaml").unwrap();
        let content = r#"
key: value
number: 42
nested:
  inner: true
"#;
        temp_file.write_all(content.as_bytes()).unwrap();

        let source = FileSource::new().add_file(temp_file.path(), None);

        let config = source.load().await.unwrap();
        assert_eq!(
            config.get_path("key").unwrap().as_string().unwrap(),
            "value"
        );
        assert_eq!(config.get_path("number").unwrap().as_integer().unwrap(), 42);
        assert!(
            config.get_path("nested.inner").unwrap().as_bool().unwrap()
        );
    }

    #[tokio::test]
    async fn test_namespace() {
        let mut temp_file = NamedTempFile::with_suffix(".json").unwrap();
        let content = r#"{"key": "value"}"#;
        temp_file.write_all(content.as_bytes()).unwrap();

        let source = FileSource::new().add_file(temp_file.path(), Some("app"));

        let config = source.load().await.unwrap();
        assert_eq!(
            config.get_path("app.key").unwrap().as_string().unwrap(),
            "value"
        );
    }

    #[tokio::test]
    async fn test_optional_file() {
        let source = FileSource::new().add_optional_file("/nonexistent/file.json", None);

        let config = source.load().await.unwrap();
        assert!(matches!(config, ConfigValue::Object(_)));
    }
}
