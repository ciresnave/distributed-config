//! Configuration validation using JSON Schema

use crate::error::{ConfigError, Result};
use crate::value::ConfigValue;
use jsonschema::{Draft, JSONSchema};
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use tracing::{debug, info};

/// Schema validator for configuration values
pub struct SchemaValidator {
    schemas: HashMap<String, JSONSchema>,
}

impl SchemaValidator {
    /// Create a new schema validator
    pub fn new() -> Self {
        Self {
            schemas: HashMap::new(),
        }
    }

    /// Add a schema for a specific configuration path
    pub fn add_schema<T>(mut self, path: &str) -> Self
    where
        T: serde::Serialize + for<'de> serde::Deserialize<'de>,
    {
        // Generate JSON schema from the type
        if let Ok(schema) = generate_schema_for_type::<T>() {
            if let Ok(compiled) = JSONSchema::compile(&schema) {
                self.schemas.insert(path.to_string(), compiled);
                info!("Added schema for configuration path: {}", path);
            }
        }
        self
    }

    /// Add a schema from a JSON schema object
    pub fn add_schema_from_json(mut self, path: &str, schema: JsonValue) -> Result<Self> {
        let compiled = JSONSchema::options()
            .with_draft(Draft::Draft7)
            .compile(&schema)
            .map_err(|e| ConfigError::ValidationError(format!("Invalid schema: {e}")))?;

        self.schemas.insert(path.to_string(), compiled);
        info!("Added JSON schema for configuration path: {}", path);
        Ok(self)
    }

    /// Add a schema from a JSON schema string
    pub fn add_schema_from_string(self, path: &str, schema_str: &str) -> Result<Self> {
        let schema: JsonValue = serde_json::from_str(schema_str)
            .map_err(|e| ConfigError::ValidationError(format!("Invalid schema JSON: {e}")))?;

        self.add_schema_from_json(path, schema)
    }

    /// Validate a configuration value against all applicable schemas
    pub fn validate(&self, config: &ConfigValue) -> Result<()> {
        // Convert ConfigValue to JSON for validation
        let json_value = config_value_to_json(config)?;

        let mut validation_errors = Vec::new();

        // Validate against each schema
        for (path, schema) in &self.schemas {
            if let Some(value_to_validate) = get_value_at_path(&json_value, path) {
                if let Err(errors) = schema.validate(&value_to_validate) {
                    for error in errors {
                        validation_errors.push(format!("Path '{path}': {error}"));
                    }
                }
            } else {
                debug!("No value found at path '{}' for validation", path);
            }
        }

        if !validation_errors.is_empty() {
            return Err(ConfigError::ValidationError(validation_errors.join("; ")));
        }

        debug!(
            "Configuration validation passed for {} schemas",
            self.schemas.len()
        );
        Ok(())
    }

    /// Validate a specific configuration value at a path
    pub fn validate_path(&self, path: &str, value: &ConfigValue) -> Result<()> {
        if let Some(schema) = self.schemas.get(path) {
            let json_value = config_value_to_json(value)?;

            let result = schema.validate(&json_value);
            if let Err(errors) = result {
                let error_messages: Vec<String> = errors.map(|e| e.to_string()).collect();
                return Err(ConfigError::ValidationError(error_messages.join("; ")));
            }
        }

        Ok(())
    }

    /// Get the list of schema paths
    pub fn schema_paths(&self) -> Vec<String> {
        self.schemas.keys().cloned().collect()
    }

    /// Check if a schema exists for a given path
    pub fn has_schema(&self, path: &str) -> bool {
        self.schemas.contains_key(path)
    }

    /// Remove a schema for a path
    pub fn remove_schema(&mut self, path: &str) -> bool {
        self.schemas.remove(path).is_some()
    }
}

impl Default for SchemaValidator {
    fn default() -> Self {
        Self::new()
    }
}

/// Convert ConfigValue to JSON Value for validation
fn config_value_to_json(config: &ConfigValue) -> Result<JsonValue> {
    match config {
        ConfigValue::Null => Ok(JsonValue::Null),
        ConfigValue::Bool(b) => Ok(JsonValue::Bool(*b)),
        ConfigValue::Integer(i) => Ok(JsonValue::Number((*i).into())),
        ConfigValue::Float(f) => {
            if let Some(num) = serde_json::Number::from_f64(*f) {
                Ok(JsonValue::Number(num))
            } else {
                Ok(JsonValue::Null)
            }
        }
        ConfigValue::String(s) => Ok(JsonValue::String(s.clone())),
        ConfigValue::Array(arr) => {
            let json_arr: Result<Vec<JsonValue>> = arr.iter().map(config_value_to_json).collect();
            Ok(JsonValue::Array(json_arr?))
        }
        ConfigValue::Object(obj) => {
            let json_obj: Result<serde_json::Map<String, JsonValue>> = obj
                .iter()
                .map(|(k, v)| config_value_to_json(v).map(|json_v| (k.clone(), json_v)))
                .collect();
            Ok(JsonValue::Object(json_obj?))
        }
        ConfigValue::Duration(d) => Ok(JsonValue::Number(d.as_secs().into())),
    }
}

/// Get a value at a specific path in a JSON object
fn get_value_at_path(json: &JsonValue, path: &str) -> Option<JsonValue> {
    if path.is_empty() {
        return Some(json.clone());
    }

    let parts: Vec<&str> = path.split('.').collect();
    let mut current = json;

    for part in parts {
        match current {
            JsonValue::Object(obj) => {
                current = obj.get(part)?;
            }
            _ => return None,
        }
    }

    Some(current.clone())
}

/// Generate a JSON schema for a Rust type
fn generate_schema_for_type<T>() -> Result<JsonValue>
where
    T: serde::Serialize + for<'de> serde::Deserialize<'de>,
{
    // This is a simplified schema generator
    // In a real implementation, you might want to use a crate like `schemars`

    // For now, we'll create a basic schema structure
    let schema = serde_json::json!({
        "$schema": "http://json-schema.org/draft-07/schema#",
        "type": "object",
        "properties": {},
        "additionalProperties": true
    });

    Ok(schema)
}

/// Common validation schemas
pub mod schemas {
    use super::*;

    /// Database configuration schema
    pub fn database_config() -> JsonValue {
        serde_json::json!({
            "$schema": "http://json-schema.org/draft-07/schema#",
            "type": "object",
            "properties": {
                "host": {
                    "type": "string",
                    "format": "hostname"
                },
                "port": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": 65535
                },
                "username": {
                    "type": "string",
                    "minLength": 1
                },
                "password": {
                    "type": "string",
                    "minLength": 1
                },
                "database": {
                    "type": "string",
                    "minLength": 1
                },
                "max_connections": {
                    "type": "integer",
                    "minimum": 1
                },
                "timeout": {
                    "type": "integer",
                    "minimum": 0
                }
            },
            "required": ["host", "port", "username", "password", "database"],
            "additionalProperties": false
        })
    }

    /// Server configuration schema
    pub fn server_config() -> JsonValue {
        serde_json::json!({
            "$schema": "http://json-schema.org/draft-07/schema#",
            "type": "object",
            "properties": {
                "host": {
                    "type": "string",
                    "default": "0.0.0.0"
                },
                "port": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": 65535
                },
                "workers": {
                    "type": "integer",
                    "minimum": 1
                },
                "debug": {
                    "type": "boolean",
                    "default": false
                },
                "log_level": {
                    "type": "string",
                    "enum": ["trace", "debug", "info", "warn", "error"]
                }
            },
            "required": ["port"],
            "additionalProperties": false
        })
    }

    /// Feature flags schema
    pub fn feature_flags() -> JsonValue {
        serde_json::json!({
            "$schema": "http://json-schema.org/draft-07/schema#",
            "type": "object",
            "patternProperties": {
                "^[a-zA-Z][a-zA-Z0-9_-]*$": {
                    "type": "boolean"
                }
            },
            "additionalProperties": false
        })
    }

    /// API configuration schema
    pub fn api_config() -> JsonValue {
        serde_json::json!({
            "$schema": "http://json-schema.org/draft-07/schema#",
            "type": "object",
            "properties": {
                "base_url": {
                    "type": "string",
                    "format": "uri"
                },
                "timeout": {
                    "type": "integer",
                    "minimum": 0
                },
                "retries": {
                    "type": "integer",
                    "minimum": 0,
                    "maximum": 10
                },
                "rate_limit": {
                    "type": "object",
                    "properties": {
                        "requests_per_second": {
                            "type": "integer",
                            "minimum": 1
                        },
                        "burst_size": {
                            "type": "integer",
                            "minimum": 1
                        }
                    },
                    "additionalProperties": false
                }
            },
            "required": ["base_url"],
            "additionalProperties": false
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_schema_validator_basic() {
        let mut validator = SchemaValidator::new();

        // Add a simple schema
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "name": {"type": "string"},
                "age": {"type": "integer", "minimum": 0}
            },
            "required": ["name"]
        });

        validator = validator.add_schema_from_json("person", schema).unwrap();

        // Valid configuration
        let mut config = HashMap::new();
        config.insert("name".to_string(), ConfigValue::String("John".to_string()));
        config.insert("age".to_string(), ConfigValue::Integer(25));
        let valid_config = ConfigValue::Object(config);

        assert!(validator.validate_path("person", &valid_config).is_ok());

        // Invalid configuration (missing required field)
        let mut invalid_config = HashMap::new();
        invalid_config.insert("age".to_string(), ConfigValue::Integer(25));
        let invalid_config = ConfigValue::Object(invalid_config);

        assert!(validator.validate_path("person", &invalid_config).is_err());
    }

    #[test]
    fn test_database_schema() {
        let mut validator = SchemaValidator::new();
        validator = validator
            .add_schema_from_json("database", schemas::database_config())
            .unwrap();

        // Valid database config
        let mut config = HashMap::new();
        config.insert(
            "host".to_string(),
            ConfigValue::String("localhost".to_string()),
        );
        config.insert("port".to_string(), ConfigValue::Integer(5432));
        config.insert(
            "username".to_string(),
            ConfigValue::String("user".to_string()),
        );
        config.insert(
            "password".to_string(),
            ConfigValue::String("pass".to_string()),
        );
        config.insert(
            "database".to_string(),
            ConfigValue::String("mydb".to_string()),
        );
        let valid_config = ConfigValue::Object(config);

        assert!(validator.validate_path("database", &valid_config).is_ok());

        // Invalid database config (invalid port)
        let mut invalid_config = HashMap::new();
        invalid_config.insert(
            "host".to_string(),
            ConfigValue::String("localhost".to_string()),
        );
        invalid_config.insert("port".to_string(), ConfigValue::Integer(70000)); // Invalid port
        invalid_config.insert(
            "username".to_string(),
            ConfigValue::String("user".to_string()),
        );
        invalid_config.insert(
            "password".to_string(),
            ConfigValue::String("pass".to_string()),
        );
        invalid_config.insert(
            "database".to_string(),
            ConfigValue::String("mydb".to_string()),
        );
        let invalid_config = ConfigValue::Object(invalid_config);

        assert!(validator
            .validate_path("database", &invalid_config)
            .is_err());
    }

    #[test]
    fn test_feature_flags_schema() {
        let mut validator = SchemaValidator::new();
        validator = validator
            .add_schema_from_json("feature_flags", schemas::feature_flags())
            .unwrap();

        // Valid feature flags
        let mut config = HashMap::new();
        config.insert("new_ui".to_string(), ConfigValue::Bool(true));
        config.insert("beta_feature".to_string(), ConfigValue::Bool(false));
        let valid_config = ConfigValue::Object(config);

        assert!(validator
            .validate_path("feature_flags", &valid_config)
            .is_ok());

        // Invalid feature flags (non-boolean value)
        let mut invalid_config = HashMap::new();
        invalid_config.insert(
            "new_ui".to_string(),
            ConfigValue::String("true".to_string()),
        );
        let invalid_config = ConfigValue::Object(invalid_config);

        assert!(validator
            .validate_path("feature_flags", &invalid_config)
            .is_err());
    }

    #[test]
    fn test_get_value_at_path() {
        let json = serde_json::json!({
            "app": {
                "database": {
                    "host": "localhost",
                    "port": 5432
                }
            }
        });

        assert_eq!(
            get_value_at_path(&json, "app.database.host"),
            Some(serde_json::json!("localhost"))
        );

        assert_eq!(
            get_value_at_path(&json, "app.database.port"),
            Some(serde_json::json!(5432))
        );

        assert_eq!(get_value_at_path(&json, "app.nonexistent"), None);
    }
}
