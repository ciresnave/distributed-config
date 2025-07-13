//! Configuration sources for loading configuration from various locations

use crate::error::Result;
use crate::value::ConfigValue;
use async_trait::async_trait;
use std::collections::HashMap;

pub mod env;
pub mod file;
pub mod remote;

pub use env::EnvSource;
pub use file::FileSource;
pub use remote::RemoteSource;

/// Trait for configuration sources
#[async_trait]
pub trait ConfigSource: Send + Sync {
    /// Load configuration from this source
    async fn load(&self) -> Result<ConfigValue>;

    /// Get the name of this source (for debugging/logging)
    fn name(&self) -> &str;

    /// Check if this source supports watching for changes
    fn supports_watching(&self) -> bool {
        false
    }

    /// Start watching for changes (if supported)
    async fn start_watching(&self) -> Result<tokio::sync::mpsc::Receiver<ConfigValue>> {
        Err(crate::error::ConfigError::Other(
            "Watching not supported by this source".to_string(),
        ))
    }
}

/// A source that combines multiple configuration sources
pub struct CompositeSource {
    sources: Vec<(Box<dyn ConfigSource>, u32)>, // (source, priority)
    name: String,
}

impl CompositeSource {
    /// Create a new composite source
    pub fn new(name: String) -> Self {
        Self {
            sources: Vec::new(),
            name,
        }
    }

    /// Add a source with priority (higher number = higher priority)
    pub fn add_source(mut self, source: Box<dyn ConfigSource>, priority: u32) -> Self {
        self.sources.push((source, priority));
        self.sources.sort_by(|a, b| a.1.cmp(&b.1)); // Sort by priority
        self
    }
}

#[async_trait]
impl ConfigSource for CompositeSource {
    async fn load(&self) -> Result<ConfigValue> {
        let mut merged_config = ConfigValue::Object(HashMap::new());

        // Load from all sources in priority order (lowest to highest)
        for (source, _priority) in &self.sources {
            let config = source.load().await?;
            merged_config.merge(config);
        }

        Ok(merged_config)
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn supports_watching(&self) -> bool {
        self.sources
            .iter()
            .any(|(source, _)| source.supports_watching())
    }
}

/// Helper function to merge two ConfigValue objects
pub fn merge_config_values(mut base: ConfigValue, overlay: ConfigValue) -> ConfigValue {
    base.merge(overlay);
    base
}
