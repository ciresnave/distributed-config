//! Environment variable configuration source

use crate::error::{ConfigError, Result};
use crate::sources::ConfigSource;
use crate::value::ConfigValue;
use async_trait::async_trait;
use std::collections::HashMap;
use std::env;
use tracing::{debug, info};

/// Configuration source that loads from environment variables
pub struct EnvSource {
    prefix: Option<String>,
    separator: String,
    case_sensitive: bool,
    name: String,
}

impl EnvSource {
    /// Create a new environment source
    pub fn new() -> Self {
        Self {
            prefix: None,
            separator: "__".to_string(),
            case_sensitive: false,
            name: "env".to_string(),
        }
    }

    /// Set a prefix for environment variables (e.g., "APP_")
    pub fn prefix<S: Into<String>>(mut self, prefix: S) -> Self {
        self.prefix = Some(prefix.into());
        self
    }

    /// Set the separator used to create nested keys (default: "__")
    pub fn separator<S: Into<String>>(mut self, separator: S) -> Self {
        self.separator = separator.into();
        self
    }

    /// Set whether environment variable names are case-sensitive
    pub fn case_sensitive(mut self, case_sensitive: bool) -> Self {
        self.case_sensitive = case_sensitive;
        self
    }

    /// Set the name of this source
    pub fn with_name<S: Into<String>>(mut self, name: S) -> Self {
        self.name = name.into();
        self
    }

    /// Convert an environment variable name to a configuration key
    fn env_to_key(&self, env_name: &str) -> Option<String> {
        let mut key = env_name.to_string();

        // Remove prefix if present
        if let Some(prefix) = &self.prefix {
            if key.starts_with(prefix) {
                key = key[prefix.len()..].to_string();
            } else {
                return None; // Skip variables that don't match the prefix
            }
        }

        // Convert to lowercase if not case-sensitive
        if !self.case_sensitive {
            key = key.to_lowercase();
        }

        // Replace separator with dots for nested keys
        key = key.replace(&self.separator, ".");

        Some(key)
    }

    /// Parse environment variable value into ConfigValue
    fn parse_env_value(&self, value: &str) -> ConfigValue {
        // Try to parse as various types

        // Boolean
        match value.to_lowercase().as_str() {
            "true" | "yes" | "1" | "on" => return ConfigValue::Bool(true),
            "false" | "no" | "0" | "off" => return ConfigValue::Bool(false),
            _ => {}
        }

        // Integer
        if let Ok(int_val) = value.parse::<i64>() {
            return ConfigValue::Integer(int_val);
        }

        // Float
        if let Ok(float_val) = value.parse::<f64>() {
            return ConfigValue::Float(float_val);
        }

        // JSON (for complex types)
        if value.starts_with('{') && value.ends_with('}') {
            if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(value) {
                return ConfigValue::from(json_val);
            }
        }

        // JSON Array
        if value.starts_with('[') && value.ends_with(']') {
            if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(value) {
                return ConfigValue::from(json_val);
            }
        }

        // Comma-separated list
        if value.contains(',') {
            let items: Vec<ConfigValue> = value
                .split(',')
                .map(|s| self.parse_env_value(s.trim()))
                .collect();
            return ConfigValue::Array(items);
        }

        // Default to string
        ConfigValue::String(value.to_string())
    }
}

impl Default for EnvSource {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ConfigSource for EnvSource {
    async fn load(&self) -> Result<ConfigValue> {
        let mut config = ConfigValue::Object(HashMap::new());

        info!("Loading configuration from environment variables");

        // Get all environment variables
        let env_vars: Vec<(String, String)> = env::vars().collect();
        let mut processed_count = 0;

        for (env_name, env_value) in env_vars {
            if let Some(config_key) = self.env_to_key(&env_name) {
                let config_value = self.parse_env_value(&env_value);

                debug!(
                    "Mapping env var {} -> {}: {:?}",
                    env_name, config_key, config_value
                );

                if let Err(e) = config.set_path(&config_key, config_value) {
                    return Err(ConfigError::Other(format!(
                        "Failed to set config path '{config_key}' from env var '{env_name}': {e}"
                    )));
                }

                processed_count += 1;
            }
        }

        info!("Processed {} environment variables", processed_count);

        Ok(config)
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn supports_watching(&self) -> bool {
        false // Environment variables don't typically change during runtime
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[tokio::test]
    async fn test_env_source_basic() {
        unsafe {
            env::set_var("TEST_KEY", "test_value");
            env::set_var("TEST_NUMBER", "42");
            env::set_var("TEST_BOOL", "true");
        }

        let source = EnvSource::new().prefix("TEST_");
        let config = source.load().await.unwrap();

        assert_eq!(
            config.get_path("key").unwrap().as_string().unwrap(),
            "test_value"
        );
        assert_eq!(config.get_path("number").unwrap().as_integer().unwrap(), 42);
        assert!(config.get_path("bool").unwrap().as_bool().unwrap());

        // Clean up
        unsafe {
            env::remove_var("TEST_KEY");
            env::remove_var("TEST_NUMBER");
            env::remove_var("TEST_BOOL");
        }
    }

    #[tokio::test]
    async fn test_env_source_nested() {
        unsafe {
            env::set_var("APP_DATABASE__HOST", "localhost");
            env::set_var("APP_DATABASE__PORT", "5432");
        }

        let source = EnvSource::new().prefix("APP_").separator("__");
        let config = source.load().await.unwrap();

        assert_eq!(
            config
                .get_path("database.host")
                .unwrap()
                .as_string()
                .unwrap(),
            "localhost"
        );
        assert_eq!(
            config
                .get_path("database.port")
                .unwrap()
                .as_integer()
                .unwrap(),
            5432
        );

        // Clean up
        unsafe {
            env::remove_var("APP_DATABASE__HOST");
            env::remove_var("APP_DATABASE__PORT");
        }
    }

    #[tokio::test]
    async fn test_env_source_array() {
        unsafe {
            env::set_var("TEST_ARRAY", "item1,item2,item3");
        }

        let source = EnvSource::new().prefix("TEST_");
        let config = source.load().await.unwrap();

        let array = config.get_path("array").unwrap().as_array().unwrap();
        assert_eq!(array.len(), 3);
        assert_eq!(array[0].as_string().unwrap(), "item1");
        assert_eq!(array[1].as_string().unwrap(), "item2");
        assert_eq!(array[2].as_string().unwrap(), "item3");

        // Clean up
        unsafe {
            env::remove_var("TEST_ARRAY");
        }
    }

    #[tokio::test]
    async fn test_env_source_json() {
        unsafe {
            env::set_var("TEST_JSON", r#"{"key": "value", "number": 42}"#);
        }

        let source = EnvSource::new().prefix("TEST_");
        let config = source.load().await.unwrap();

        assert_eq!(
            config.get_path("json.key").unwrap().as_string().unwrap(),
            "value"
        );
        assert_eq!(
            config
                .get_path("json.number")
                .unwrap()
                .as_integer()
                .unwrap(),
            42
        );

        // Clean up
        unsafe {
            env::remove_var("TEST_JSON");
        }
    }
}
