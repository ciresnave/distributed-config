//! Configuration value types and conversions

use crate::error::{ConfigError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

/// A dynamic configuration value that can hold various types
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ConfigValue {
    /// Null/None value
    Null,
    /// Boolean value
    Bool(bool),
    /// Integer value
    Integer(i64),
    /// Floating point value
    Float(f64),
    /// String value
    String(String),
    /// Array of values
    Array(Vec<ConfigValue>),
    /// Object/Map of values
    Object(HashMap<String, ConfigValue>),
    /// Duration value (serialized as seconds)
    #[serde(with = "duration_serde")]
    Duration(Duration),
}

impl ConfigValue {
    /// Check if the value is null
    pub fn is_null(&self) -> bool {
        matches!(self, ConfigValue::Null)
    }

    /// Convert to boolean, returning error if conversion fails
    pub fn as_bool(&self) -> Result<bool> {
        match self {
            ConfigValue::Bool(b) => Ok(*b),
            ConfigValue::String(s) => match s.to_lowercase().as_str() {
                "true" | "yes" | "1" | "on" => Ok(true),
                "false" | "no" | "0" | "off" => Ok(false),
                _ => Err(ConfigError::TypeConversion {
                    from: "string".to_string(),
                    to: "bool".to_string(),
                }),
            },
            ConfigValue::Integer(i) => Ok(*i != 0),
            _ => Err(ConfigError::TypeConversion {
                from: format!("{self:?}"),
                to: "bool".to_string(),
            }),
        }
    }

    /// Convert to integer, returning error if conversion fails
    pub fn as_integer(&self) -> Result<i64> {
        match self {
            ConfigValue::Integer(i) => Ok(*i),
            ConfigValue::Float(f) => Ok(*f as i64),
            ConfigValue::String(s) => s.parse().map_err(|_| ConfigError::TypeConversion {
                from: "string".to_string(),
                to: "integer".to_string(),
            }),
            ConfigValue::Bool(b) => Ok(if *b { 1 } else { 0 }),
            _ => Err(ConfigError::TypeConversion {
                from: format!("{self:?}"),
                to: "integer".to_string(),
            }),
        }
    }

    /// Convert to float, returning error if conversion fails
    pub fn as_float(&self) -> Result<f64> {
        match self {
            ConfigValue::Float(f) => Ok(*f),
            ConfigValue::Integer(i) => Ok(*i as f64),
            ConfigValue::String(s) => s.parse().map_err(|_| ConfigError::TypeConversion {
                from: "string".to_string(),
                to: "float".to_string(),
            }),
            _ => Err(ConfigError::TypeConversion {
                from: format!("{self:?}"),
                to: "float".to_string(),
            }),
        }
    }

    /// Convert to string
    pub fn as_string(&self) -> Result<String> {
        match self {
            ConfigValue::String(s) => Ok(s.clone()),
            ConfigValue::Integer(i) => Ok(i.to_string()),
            ConfigValue::Float(f) => Ok(f.to_string()),
            ConfigValue::Bool(b) => Ok(b.to_string()),
            _ => Err(ConfigError::TypeConversion {
                from: format!("{self:?}"),
                to: "string".to_string(),
            }),
        }
    }

    /// Convert to array, returning error if conversion fails
    pub fn as_array(&self) -> Result<&Vec<ConfigValue>> {
        match self {
            ConfigValue::Array(arr) => Ok(arr),
            _ => Err(ConfigError::TypeConversion {
                from: format!("{self:?}"),
                to: "array".to_string(),
            }),
        }
    }

    /// Convert to object/map, returning error if conversion fails
    pub fn as_object(&self) -> Result<&HashMap<String, ConfigValue>> {
        match self {
            ConfigValue::Object(obj) => Ok(obj),
            _ => Err(ConfigError::TypeConversion {
                from: format!("{self:?}"),
                to: "object".to_string(),
            }),
        }
    }

    /// Convert to duration, returning error if conversion fails
    pub fn as_duration(&self) -> Result<Duration> {
        match self {
            ConfigValue::Duration(d) => Ok(*d),
            ConfigValue::Integer(i) => Ok(Duration::from_secs(*i as u64)),
            ConfigValue::String(s) => {
                // Try parsing as seconds first
                if let Ok(secs) = s.parse::<u64>() {
                    return Ok(Duration::from_secs(secs));
                }

                // Try parsing duration strings like "30s", "5m", "1h"
                parse_duration_string(s)
            }
            _ => Err(ConfigError::TypeConversion {
                from: format!("{self:?}"),
                to: "duration".to_string(),
            }),
        }
    }

    /// Get a nested value by dot-separated path
    pub fn get_path(&self, path: &str) -> Option<&ConfigValue> {
        if path.is_empty() {
            return Some(self);
        }

        let parts: Vec<&str> = path.split('.').collect();
        let mut current = self;

        for part in parts {
            match current {
                ConfigValue::Object(obj) => {
                    current = obj.get(part)?;
                }
                _ => return None,
            }
        }

        Some(current)
    }

    /// Set a nested value by dot-separated path
    pub fn set_path(&mut self, path: &str, value: ConfigValue) -> Result<()> {
        if path.is_empty() {
            *self = value;
            return Ok(());
        }

        let parts: Vec<&str> = path.split('.').collect();
        let mut current = self;

        // Navigate to the parent of the target
        for part in &parts[..parts.len() - 1] {
            match current {
                ConfigValue::Object(obj) => {
                    current = obj
                        .entry(part.to_string())
                        .or_insert_with(|| ConfigValue::Object(HashMap::new()));
                }
                _ => {
                    return Err(ConfigError::Other(
                        "Cannot set path on non-object value".to_string(),
                    ));
                }
            }
        }

        // Set the final value
        if let ConfigValue::Object(obj) = current {
            obj.insert(parts[parts.len() - 1].to_string(), value);
            Ok(())
        } else {
            Err(ConfigError::Other(
                "Cannot set path on non-object value".to_string(),
            ))
        }
    }

    /// Merge another ConfigValue into this one
    pub fn merge(&mut self, other: ConfigValue) {
        match (self, other) {
            (ConfigValue::Object(left), ConfigValue::Object(right)) => {
                for (key, value) in right {
                    if let Some(existing) = left.get_mut(&key) {
                        existing.merge(value);
                    } else {
                        left.insert(key, value);
                    }
                }
            }
            (left, right) => *left = right,
        }
    }
}

// Implement From traits for convenient creation
impl From<bool> for ConfigValue {
    fn from(value: bool) -> Self {
        ConfigValue::Bool(value)
    }
}

impl From<i64> for ConfigValue {
    fn from(value: i64) -> Self {
        ConfigValue::Integer(value)
    }
}

impl From<i32> for ConfigValue {
    fn from(value: i32) -> Self {
        ConfigValue::Integer(value as i64)
    }
}

impl From<u32> for ConfigValue {
    fn from(value: u32) -> Self {
        ConfigValue::Integer(value as i64)
    }
}

impl From<f64> for ConfigValue {
    fn from(value: f64) -> Self {
        ConfigValue::Float(value)
    }
}

impl From<String> for ConfigValue {
    fn from(value: String) -> Self {
        ConfigValue::String(value)
    }
}

impl From<&str> for ConfigValue {
    fn from(value: &str) -> Self {
        ConfigValue::String(value.to_string())
    }
}

impl From<Duration> for ConfigValue {
    fn from(value: Duration) -> Self {
        ConfigValue::Duration(value)
    }
}

impl From<Vec<ConfigValue>> for ConfigValue {
    fn from(value: Vec<ConfigValue>) -> Self {
        ConfigValue::Array(value)
    }
}

impl From<HashMap<String, ConfigValue>> for ConfigValue {
    fn from(value: HashMap<String, ConfigValue>) -> Self {
        ConfigValue::Object(value)
    }
}

impl From<serde_json::Value> for ConfigValue {
    fn from(value: serde_json::Value) -> Self {
        match value {
            serde_json::Value::Null => ConfigValue::Null,
            serde_json::Value::Bool(b) => ConfigValue::Bool(b),
            serde_json::Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    ConfigValue::Integer(i)
                } else if let Some(f) = n.as_f64() {
                    ConfigValue::Float(f)
                } else {
                    ConfigValue::Null
                }
            }
            serde_json::Value::String(s) => ConfigValue::String(s),
            serde_json::Value::Array(arr) => {
                ConfigValue::Array(arr.into_iter().map(ConfigValue::from).collect())
            }
            serde_json::Value::Object(obj) => ConfigValue::Object(
                obj.into_iter()
                    .map(|(k, v)| (k, ConfigValue::from(v)))
                    .collect(),
            ),
        }
    }
}

/// Helper function to parse duration strings like "30s", "5m", "1h"
fn parse_duration_string(s: &str) -> Result<Duration> {
    let s = s.trim();

    if s.is_empty() {
        return Err(ConfigError::TypeConversion {
            from: "empty string".to_string(),
            to: "duration".to_string(),
        });
    }

    let (number_part, unit_part) = if s.chars().last().unwrap().is_alphabetic() {
        let split_pos = s.len() - 1;
        (&s[..split_pos], &s[split_pos..])
    } else {
        (s, "s") // Default to seconds
    };

    let number: f64 = number_part
        .parse()
        .map_err(|_| ConfigError::TypeConversion {
            from: s.to_string(),
            to: "duration".to_string(),
        })?;

    let duration = match unit_part {
        "ns" => Duration::from_nanos((number * 1.0) as u64),
        "us" | "μs" => Duration::from_micros((number * 1.0) as u64),
        "ms" => Duration::from_millis((number * 1.0) as u64),
        "s" => Duration::from_secs_f64(number),
        "m" => Duration::from_secs_f64(number * 60.0),
        "h" => Duration::from_secs_f64(number * 3600.0),
        "d" => Duration::from_secs_f64(number * 86400.0),
        _ => {
            return Err(ConfigError::TypeConversion {
                from: s.to_string(),
                to: "duration".to_string(),
            });
        }
    };

    Ok(duration)
}

/// Custom serde module for Duration
mod duration_serde {
    use super::*;
    use serde::{Deserializer, Serializer};

    pub fn serialize<S>(duration: &Duration, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u64(duration.as_secs())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> std::result::Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let secs = u64::deserialize(deserializer)?;
        Ok(Duration::from_secs(secs))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_value_conversions() {
        let bool_val = ConfigValue::Bool(true);
        assert!(bool_val.as_bool().unwrap());
        assert_eq!(bool_val.as_integer().unwrap(), 1);

        let int_val = ConfigValue::Integer(42);
        assert_eq!(int_val.as_integer().unwrap(), 42);
        assert_eq!(int_val.as_float().unwrap(), 42.0);

        let str_val = ConfigValue::String("test".to_string());
        assert_eq!(str_val.as_string().unwrap(), "test");
    }

    #[test]
    fn test_path_operations() {
        let mut config = ConfigValue::Object(HashMap::new());
        config
            .set_path("app.database.host", "localhost".into())
            .unwrap();

        assert_eq!(
            config
                .get_path("app.database.host")
                .unwrap()
                .as_string()
                .unwrap(),
            "localhost"
        );
    }

    #[test]
    fn test_duration_parsing() {
        assert_eq!(
            parse_duration_string("30s").unwrap(),
            Duration::from_secs(30)
        );
        assert_eq!(
            parse_duration_string("5m").unwrap(),
            Duration::from_secs(300)
        );
        assert_eq!(
            parse_duration_string("1h").unwrap(),
            Duration::from_secs(3600)
        );
        assert_eq!(
            parse_duration_string("2d").unwrap(),
            Duration::from_secs(172800)
        );
    }
}
