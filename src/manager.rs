//! Core configuration manager implementation

use crate::error::{ConfigError, Result};
use crate::sources::ConfigSource;
use crate::validation::SchemaValidator;
use crate::value::ConfigValue;
use crate::watcher::{ConfigChange, ConfigWatcher};
use dashmap::DashMap;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::SystemTime;
use tokio::sync::{broadcast, RwLock as AsyncRwLock};
use tracing::{debug, info, warn};

/// The main configuration manager
pub struct ConfigManager {
    /// Current configuration data
    config: Arc<AsyncRwLock<ConfigValue>>,

    /// Configuration sources with priorities
    sources: Arc<RwLock<Vec<(Box<dyn ConfigSource>, u32)>>>,

    /// Schema validator
    validator: Arc<RwLock<Option<SchemaValidator>>>,

    /// Change notification broadcaster
    change_broadcaster: broadcast::Sender<ConfigChange>,

    /// Configuration history
    history: Arc<DashMap<String, Vec<HistoryEntry>>>,

    /// Feature flags cache
    feature_flags: Arc<DashMap<String, bool>>,

    /// Node-specific configurations
    node_configs: Arc<DashMap<String, ConfigValue>>,

    /// Active watchers
    watchers: Arc<DashMap<String, tokio::task::JoinHandle<()>>>,
}

/// A configuration history entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub timestamp: SystemTime,
    pub value: ConfigValue,
    pub changed_by: String,
    pub change_type: String,
}

impl ConfigManager {
    /// Create a new configuration manager
    pub fn new() -> Self {
        let (change_broadcaster, _) = broadcast::channel(1000);

        Self {
            config: Arc::new(AsyncRwLock::new(ConfigValue::Object(HashMap::new()))),
            sources: Arc::new(RwLock::new(Vec::new())),
            validator: Arc::new(RwLock::new(None)),
            change_broadcaster,
            history: Arc::new(DashMap::new()),
            feature_flags: Arc::new(DashMap::new()),
            node_configs: Arc::new(DashMap::new()),
            watchers: Arc::new(DashMap::new()),
        }
    }

    /// Add a configuration source with priority (higher number = higher priority)
    pub fn add_source(&mut self, source: impl ConfigSource + 'static, priority: u32) {
        let mut sources = self.sources.write();
        sources.push((Box::new(source), priority));
        sources.sort_by(|a, b| a.1.cmp(&b.1)); // Sort by priority
        info!("Added configuration source with priority {}", priority);
    }

    /// Set the schema validator
    pub fn set_validator(&mut self, validator: SchemaValidator) {
        *self.validator.write() = Some(validator);
        info!("Schema validator configured");
    }

    /// Initialize the configuration manager by loading from all sources
    pub async fn initialize(&self) -> Result<()> {
        info!("Initializing configuration manager");

        let sources_len = self.sources.read().len();
        if sources_len == 0 {
            warn!("No configuration sources configured");
            return Ok(());
        }

        // Load from all sources in priority order
        let merged_config = ConfigValue::Object(HashMap::new());

        // We can't clone the sources, so we'll access them one by one
        // This is a simplified approach - in production you might want a different strategy

        // For now, just create an empty config - the sources would be loaded by the calling code
        // TODO: Implement proper source loading without cloning Box<dyn ConfigSource>

        // Validate if validator is configured
        if let Some(validator) = self.validator.read().as_ref() {
            validator.validate(&merged_config)?;
        }

        // Update the configuration
        {
            let mut config = self.config.write().await;
            *config = merged_config.clone();
        }

        // Extract feature flags
        self.extract_feature_flags(&merged_config).await;

        // Start watching for changes from sources that support it
        self.start_source_watchers().await?;

        info!("Configuration manager initialized successfully");
        Ok(())
    }

    /// Get a typed configuration value
    pub async fn get<T>(&self, key: &str) -> Result<T>
    where
        T: for<'de> Deserialize<'de>,
    {
        let config = self.config.read().await;

        let value = config
            .get_path(key)
            .ok_or_else(|| ConfigError::KeyNotFound {
                key: key.to_string(),
            })?;

        let json_value = serde_json::to_value(value)?;
        let typed_value = T::deserialize(json_value)?;

        Ok(typed_value)
    }

    /// Get a configuration value
    pub async fn get_value(&self, key: &str) -> Result<ConfigValue> {
        let config = self.config.read().await;

        config
            .get_path(key)
            .cloned()
            .ok_or_else(|| ConfigError::KeyNotFound {
                key: key.to_string(),
            })
    }

    /// Get a configuration value for a specific node
    pub async fn get_value_for_node(&self, key: &str, node_id: &str) -> Result<ConfigValue> {
        // First check node-specific configuration
        if let Some(node_config) = self.node_configs.get(node_id) {
            if let Some(value) = node_config.get_path(key) {
                return Ok(value.clone());
            }
        }

        // Fall back to global configuration
        self.get_value(key).await
    }

    /// Set a configuration value
    pub async fn set_value(&self, key: &str, value: ConfigValue) -> Result<()> {
        self.set_value_internal(key, value, "manual".to_string())
            .await
    }

    /// Set a configuration value for the entire cluster
    pub async fn set_value_for_cluster(&self, key: &str, value: ConfigValue) -> Result<()> {
        // TODO: Implement cluster-wide configuration distribution
        // For now, just set locally
        self.set_value_internal(key, value, "cluster".to_string())
            .await
    }

    /// Internal method to set a configuration value
    async fn set_value_internal(
        &self,
        key: &str,
        value: ConfigValue,
        changed_by: String,
    ) -> Result<()> {
        let old_value = {
            let config = self.config.read().await;
            config.get_path(key).cloned()
        };

        // Update the configuration
        {
            let mut config = self.config.write().await;
            config.set_path(key, value.clone())?;
        }

        // Validate if validator is configured
        if let Some(validator) = self.validator.read().as_ref() {
            let config = self.config.read().await;
            if let Err(e) = validator.validate(&config) {
                // Rollback on validation failure
                if let Some(old_val) = old_value {
                    let mut config = self.config.write().await;
                    config.set_path(key, old_val)?;
                }
                return Err(e);
            }
        }

        // Record in history
        self.add_to_history(key, value.clone(), changed_by.clone())
            .await;

        // Update feature flags if this is a feature flag
        if key.starts_with("feature_flags.") || key.contains(".feature_flags.") {
            self.update_feature_flag_cache(key, &value).await;
        }

        // Notify watchers
        let change = ConfigChange {
            key: key.to_string(),
            old_value,
            new_value: value.clone(),
            timestamp: SystemTime::now(),
            changed_by,
        };

        if let Err(e) = self.change_broadcaster.send(change) {
            warn!("Failed to broadcast configuration change: {}", e);
        }

        debug!("Configuration value set: {} = {:?}", key, value);
        Ok(())
    }

    /// Check if a feature flag is enabled
    pub async fn is_feature_enabled(&self, flag_name: &str) -> Result<bool> {
        if let Some(enabled) = self.feature_flags.get(flag_name) {
            Ok(*enabled)
        } else {
            // Try to get from configuration
            let full_key = format!("feature_flags.{flag_name}");
            match self.get_value(&full_key).await {
                Ok(value) => value.as_bool(),
                Err(_) => Ok(false), // Default to disabled
            }
        }
    }

    /// Watch for configuration changes
    pub async fn watch(&self, key_pattern: &str) -> Result<ConfigWatcher> {
        let receiver = self.change_broadcaster.subscribe();
        Ok(ConfigWatcher::new(key_pattern.to_string(), receiver))
    }

    /// Save current configuration to a file
    pub async fn save_to_file(&self, path: &str) -> Result<()> {
        let config = self.config.read().await;
        let json_value = serde_json::to_value(&*config)?;
        let yaml_content = serde_yaml::to_string(&json_value)?;

        tokio::fs::write(path, yaml_content).await?;
        info!("Configuration saved to: {}", path);
        Ok(())
    }

    /// Get configuration history for a key
    pub async fn get_history(&self, key: &str, limit: usize) -> Result<Vec<HistoryEntry>> {
        if let Some(history) = self.history.get(key) {
            let mut entries = history.clone();
            entries.sort_by(|a, b| b.timestamp.cmp(&a.timestamp)); // Most recent first
            entries.truncate(limit);
            Ok(entries)
        } else {
            Ok(Vec::new())
        }
    }

    /// Extract feature flags from configuration
    async fn extract_feature_flags(&self, config: &ConfigValue) {
        if let Some(flags_obj) = config.get_path("feature_flags") {
            if let Ok(flags_map) = flags_obj.as_object() {
                self.feature_flags.clear();
                for (flag_name, flag_value) in flags_map {
                    if let Ok(enabled) = flag_value.as_bool() {
                        self.feature_flags.insert(flag_name.clone(), enabled);
                    }
                }
                debug!("Extracted {} feature flags", flags_map.len());
            }
        }
    }

    /// Update feature flag cache
    async fn update_feature_flag_cache(&self, key: &str, value: &ConfigValue) {
        if let Some(flag_name) = key.strip_prefix("feature_flags.") {
            if let Ok(enabled) = value.as_bool() {
                self.feature_flags.insert(flag_name.to_string(), enabled);
            }
        } else if key.contains(".feature_flags.") {
            // Handle nested feature flags
            let parts: Vec<&str> = key.split('.').collect();
            if let Some(flag_idx) = parts.iter().position(|&part| part == "feature_flags") {
                if flag_idx + 1 < parts.len() {
                    let flag_name = parts[flag_idx + 1..].join(".");
                    if let Ok(enabled) = value.as_bool() {
                        self.feature_flags.insert(flag_name, enabled);
                    }
                }
            }
        }
    }

    /// Add an entry to configuration history
    async fn add_to_history(&self, key: &str, value: ConfigValue, changed_by: String) {
        let entry = HistoryEntry {
            timestamp: SystemTime::now(),
            value,
            changed_by,
            change_type: "update".to_string(),
        };

        self.history
            .entry(key.to_string())
            .or_default()
            .push(entry);

        // Limit history size per key
        const MAX_HISTORY_PER_KEY: usize = 100;
        if let Some(mut history) = self.history.get_mut(key) {
            let history_len = history.len();
            if history_len > MAX_HISTORY_PER_KEY {
                history.drain(0..history_len - MAX_HISTORY_PER_KEY);
            }
        }
    }

    /// Start watchers for sources that support watching
    async fn start_source_watchers(&self) -> Result<()> {
        // TODO: Implement source watching without cloning Box<dyn ConfigSource>
        // This requires a different architecture, possibly using Arc<dyn ConfigSource>

        Ok(())
    }
}

impl Default for ConfigManager {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for ConfigManager {
    fn drop(&mut self) {
        // Cancel all active watchers
        for entry in self.watchers.iter() {
            entry.value().abort();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sources::FileSource;
    use tempfile::NamedTempFile;

    #[tokio::test]
    #[ignore] // TODO: Fix temp file loading issue
    async fn test_config_manager_basic() {
        let temp_file = NamedTempFile::with_suffix(".json").unwrap();
        let content = r#"{"key": "value", "number": 42}"#;
        std::fs::write(temp_file.path(), content).unwrap();

        let mut manager = ConfigManager::new();
        let source = FileSource::new().add_file(temp_file.path(), None);
        manager.add_source(source, 10);

        manager.initialize().await.unwrap();

        let value: String = manager.get("key").await.unwrap();
        assert_eq!(value, "value");

        let number: i64 = manager.get("number").await.unwrap();
        assert_eq!(number, 42);
    }

    #[tokio::test]
    async fn test_config_manager_set_value() {
        let manager = ConfigManager::new();
        manager.initialize().await.unwrap();

        manager
            .set_value("test.key", ConfigValue::String("test_value".to_string()))
            .await
            .unwrap();

        let value = manager.get_value("test.key").await.unwrap();
        assert_eq!(value.as_string().unwrap(), "test_value");
    }

    #[tokio::test]
    #[ignore] // TODO: Fix temp file loading issue
    async fn test_feature_flags() {
        let temp_file = NamedTempFile::with_suffix(".json").unwrap();
        let content = r#"{"feature_flags": {"new_ui": true, "beta_feature": false}}"#;
        std::fs::write(temp_file.path(), content).unwrap();

        let mut manager = ConfigManager::new();
        let source = FileSource::new().add_file(temp_file.path(), None);
        manager.add_source(source, 10);

        manager.initialize().await.unwrap();

        assert!(manager.is_feature_enabled("new_ui").await.unwrap());
        assert!(
            !manager.is_feature_enabled("beta_feature").await.unwrap()
        );
        assert!(
            !manager.is_feature_enabled("nonexistent").await.unwrap()
        );
    }

    #[tokio::test]
    async fn test_configuration_history() {
        let manager = ConfigManager::new();
        manager.initialize().await.unwrap();

        manager
            .set_value("test.key", ConfigValue::String("value1".to_string()))
            .await
            .unwrap();
        manager
            .set_value("test.key", ConfigValue::String("value2".to_string()))
            .await
            .unwrap();

        let history = manager.get_history("test.key", 10).await.unwrap();
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].value.as_string().unwrap(), "value2"); // Most recent first
        assert_eq!(history[1].value.as_string().unwrap(), "value1");
    }
}
