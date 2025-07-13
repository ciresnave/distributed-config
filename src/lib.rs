//! # Distributed Config
//!
//! `distributed-config` is a robust configuration management library for Rust applications
//! running in distributed environments. It provides a unified interface for loading,
//! accessing, and synchronizing configuration across multiple nodes with support for
//! dynamic updates, validation, and various backend stores.
//!
//! ## Features
//!
//! - **Hierarchical Configuration**: Organize configuration in a tree structure
//! - **Multiple Sources**: Load from files, environment variables, and remote sources
//! - **Dynamic Updates**: Real-time configuration changes with notifications
//! - **Schema Validation**: Strong typing and validation support
//! - **Distributed Sync**: Synchronize configuration across multiple nodes
//! - **Feature Flags**: Built-in feature flag management
//! - **Versioning**: Configuration history and rollback support
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use distributed_config::{ConfigManager, sources::FileSource};
//! use serde::{Deserialize, Serialize};
//!
//! #[derive(Debug, Deserialize, Serialize)]
//! struct AppConfig {
//!     host: String,
//!     port: u16,
//!     debug: bool,
//! }
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let mut config = ConfigManager::new();
//!     
//!     // Add a file source
//!     let file_source = FileSource::new().add_file("config.yaml", None);
//!     config.add_source(file_source, 10);
//!     
//!     // Initialize and load configuration
//!     config.initialize().await?;
//!     
//!     // Access typed configuration
//!     let app_config = config.get::<AppConfig>("app").await?;
//!     println!("Server running on {}:{}", app_config.host, app_config.port);
//!     
//!     Ok(())
//! }
//! ```

pub mod error;
pub mod manager;
pub mod sources;
pub mod validation;
pub mod value;
pub mod watcher;

// Future: Optional backends module for Redis, etcd support
// #[cfg(feature = "redis-backend")]
// pub mod backends;

// Re-export main types for convenient access
pub use error::{ConfigError, Result};
pub use manager::ConfigManager;
pub use sources::{ConfigSource, EnvSource, FileSource};
pub use validation::SchemaValidator;
pub use value::ConfigValue;
pub use watcher::{ConfigChange, ConfigWatcher};

#[cfg(feature = "redis-backend")]
pub use sources::RemoteSource;

/// Type alias for configuration change notifications
pub type ChangeNotification = watcher::ConfigChange;

/// Initialize tracing for the library
pub fn init_tracing() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
}
