//! Error types for the distributed-config library

use thiserror::Error;

/// Result type alias for this crate
pub type Result<T> = std::result::Result<T, ConfigError>;

/// Main error type for configuration operations
#[derive(Debug, Error)]
pub enum ConfigError {
    /// Error reading or parsing configuration file
    #[error("Configuration file error: {0}")]
    FileError(#[from] std::io::Error),

    /// Error serializing or deserializing configuration
    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    /// Error parsing YAML configuration
    #[error("YAML parsing error: {0}")]
    YamlError(#[from] serde_yaml::Error),

    /// Error parsing TOML configuration
    #[error("TOML parsing error: {0}")]
    TomlError(#[from] toml::de::Error),

    /// Configuration key not found
    #[error("Configuration key not found: {key}")]
    KeyNotFound { key: String },

    /// Type conversion error
    #[error("Type conversion error: cannot convert {from} to {to}")]
    TypeConversion { from: String, to: String },

    /// Schema validation error
    #[error("Schema validation error: {0}")]
    ValidationError(String),

    /// Network error when accessing remote configuration
    #[error("Network error: {0}")]
    NetworkError(#[from] reqwest::Error),

    /// Authentication error for remote sources
    #[error("Authentication error: {0}")]
    AuthenticationError(String),

    /// Configuration source initialization error
    #[error("Source initialization error: {0}")]
    SourceInitializationError(String),

    /// Configuration watcher error
    #[error("Watcher error: {0}")]
    WatcherError(String),

    /// File system watching error
    #[error("File system error: {0}")]
    FileSystemError(#[from] notify::Error),

    /// Distributed backend error
    #[cfg(feature = "redis-backend")]
    #[error("Redis error: {0}")]
    RedisError(#[from] redis::RedisError),

    /// Distributed backend error
    #[cfg(feature = "etcd-backend")]
    #[error("Etcd error: {0}")]
    EtcdError(#[from] etcd_client::Error),

    /// Lock acquisition timeout
    #[error("Lock timeout: failed to acquire lock within {timeout_ms}ms")]
    LockTimeout { timeout_ms: u64 },

    /// Configuration conflict in distributed environment
    #[error("Configuration conflict: {0}")]
    ConflictError(String),

    /// Generic configuration error
    #[error("Configuration error: {0}")]
    Other(String),
}

impl ConfigError {
    /// Create a new validation error
    pub fn validation_error<S: Into<String>>(msg: S) -> Self {
        ConfigError::ValidationError(msg.into())
    }

    /// Create a new authentication error
    pub fn auth_error<S: Into<String>>(msg: S) -> Self {
        ConfigError::AuthenticationError(msg.into())
    }

    /// Create a new source initialization error
    pub fn source_init_error<S: Into<String>>(msg: S) -> Self {
        ConfigError::SourceInitializationError(msg.into())
    }

    /// Create a new watcher error
    pub fn watcher_error<S: Into<String>>(msg: S) -> Self {
        ConfigError::WatcherError(msg.into())
    }

    /// Create a new conflict error
    pub fn conflict_error<S: Into<String>>(msg: S) -> Self {
        ConfigError::ConflictError(msg.into())
    }

    /// Create a new generic error
    pub fn other<S: Into<String>>(msg: S) -> Self {
        ConfigError::Other(msg.into())
    }
}

impl From<anyhow::Error> for ConfigError {
    fn from(err: anyhow::Error) -> Self {
        ConfigError::Other(err.to_string())
    }
}

impl From<jsonschema::ValidationError<'_>> for ConfigError {
    fn from(err: jsonschema::ValidationError<'_>) -> Self {
        ConfigError::ValidationError(err.to_string())
    }
}
