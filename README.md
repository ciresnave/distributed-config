# Distributed Config

[![Crates.io](https://img.shields.io/crates/v/distributed-config.svg)](https://crates.io/crates/distributed-config)
[![Documentation](https://docs.rs/distributed-config/badge.svg)](https://docs.rs/distributed-config)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](https://github.com/your-username/distributed-config)

**A robust configuration management library for Rust applications running in distributed environments.**

`distributed-config` provides a unified interface for loading, accessing, and synchronizing configuration across multiple nodes with support for dynamic updates, validation, and various backend stores.

## ✨ Features

- **🏗️ Hierarchical Configuration**: Organize configuration in a tree structure with dot-notation access
- **📁 Multiple Sources**: Load from files (JSON, YAML, TOML), environment variables, and remote HTTP endpoints
- **🔄 Dynamic Updates**: Real-time configuration changes with change notifications and watchers
- **✅ Schema Validation**: Strong typing and JSON Schema validation for configuration safety
- **🌐 Distributed Sync**: Synchronize configuration across multiple nodes (with optional backends)
- **🚩 Feature Flags**: Built-in feature flag management with real-time toggling
- **📜 Versioning**: Configuration history and rollback support
- **🔐 Secure**: Safe handling of sensitive configuration values
- **⚡ Async First**: Built on Tokio with async/await throughout
- **🎯 Type Safe**: Leverage Rust's type system for configuration validation

## 🚀 Quick Start

Add this to your `Cargo.toml`:

```toml
[dependencies]
distributed-config = "0.1"
tokio = { version = "1.0", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
```

### Basic Usage

```rust
use distributed_config::{ConfigManager, sources::FileSource};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
struct AppConfig {
    host: String,
    port: u16,
    debug: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut config = ConfigManager::new();
    
    // Add a file source
    let file_source = FileSource::new().add_file("config.yaml", None);
    config.add_source(file_source, 10);
    
    // Initialize and load configuration
    config.initialize().await?;
    
    // Access typed configuration
    let app_config: AppConfig = config.get("app").await?;
    println!("Server running on {}:{}", app_config.host, app_config.port);
    
    Ok(())
}
```

### Advanced Usage with Multiple Sources

```rust
use distributed_config::{
    ConfigManager, 
    sources::{FileSource, EnvSource, RemoteSource},
    validation::SchemaValidator
};
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Deserialize, Serialize)]
struct DatabaseConfig {
    host: String,
    port: u16,
    username: String,
    password: String,
    max_connections: u32,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut config = ConfigManager::new();
    
    // Add multiple sources with priorities (higher number = higher priority)
    
    // Base configuration from file (priority 10)
    let file_source = FileSource::new()
        .add_file("config/base.yaml", None)
        .add_optional_file("config/local.yaml", None);
    config.add_source(file_source, 10);
    
    // Environment overrides (priority 20)
    let env_source = EnvSource::new()
        .prefix("APP_")
        .separator("__");
    config.add_source(env_source, 20);
    
    // Remote configuration (priority 30)
    let remote_source = RemoteSource::new()
        .endpoint("https://config-server.example.com/config")
        .auth_token("your-token")
        .poll_interval(Duration::from_secs(30));
    config.add_source(remote_source, 30);
    
    // Add schema validation
    let validator = SchemaValidator::new()
        .add_schema_from_json(
            "database",
            distributed_config::validation::schemas::database_config()
        )?;
    config.set_validator(validator);
    
    // Initialize
    config.initialize().await?;
    
    // Access configuration
    let db_config: DatabaseConfig = config.get("database").await?;
    println!("Connecting to database at {}:{}", db_config.host, db_config.port);
    
    // Watch for changes
    let mut watcher = config.watch("database").await?;
    tokio::spawn(async move {
        while let Some(change) = watcher.next().await {
            println!("Database config changed: {}", change.key);
            // Reconnect to database with new config
        }
    });
    
    // Feature flags
    if config.is_feature_enabled("new_feature")? {
        println!("New feature is enabled!");
    }
    
    // Runtime updates
    config.set_value("database.max_connections", 20.into()).await?;
    
    Ok(())
}
```

## 📖 Configuration Sources

### File Source

Supports JSON, YAML, and TOML formats:

```rust
let file_source = FileSource::new()
    .add_file("config.yaml", None)                    // Load into root
    .add_file("database.json", Some("database"))      // Load into "database" namespace
    .add_optional_file("local.toml", None);           // Optional file (won't fail if missing)
```

### Environment Source

Maps environment variables to configuration keys:

```rust
let env_source = EnvSource::new()
    .prefix("MYAPP_")                    // Only variables starting with MYAPP_
    .separator("__")                     // MYAPP_DATABASE__HOST -> database.host
    .case_sensitive(false);              // Convert to lowercase
```

Examples:
- `MYAPP_DATABASE__HOST=localhost` → `database.host = "localhost"`
- `MYAPP_SERVER__PORT=8080` → `server.port = 8080`
- `MYAPP_DEBUG=true` → `debug = true`

### Remote Source

Load configuration from HTTP endpoints:

```rust
let remote_source = RemoteSource::new()
    .endpoint("https://config.example.com/api/config")
    .auth_token("bearer-token")
    .timeout(Duration::from_secs(10))
    .poll_interval(Duration::from_secs(30))    // Check for updates every 30s
    .header("X-Environment", "production");
```

## ✅ Validation

Use JSON Schema validation to ensure configuration correctness:

```rust
use distributed_config::validation::{SchemaValidator, schemas};

let validator = SchemaValidator::new()
    .add_schema_from_json("database", schemas::database_config())?
    .add_schema_from_json("server", schemas::server_config())?
    .add_schema_from_string("custom", r#"
        {
            "type": "object",
            "properties": {
                "name": {"type": "string"},
                "count": {"type": "integer", "minimum": 0}
            },
            "required": ["name"]
        }
    "#)?;

config.set_validator(validator);
```

Built-in schemas available:
- `schemas::database_config()` - Database connection settings
- `schemas::server_config()` - Server/HTTP settings  
- `schemas::api_config()` - API client settings
- `schemas::feature_flags()` - Feature flag definitions

## 🔄 Dynamic Updates & Watching

Monitor configuration changes in real-time:

```rust
// Watch specific keys
let mut db_watcher = config.watch("database").await?;
let mut feature_watcher = config.watch("feature_flags.*").await?;

tokio::spawn(async move {
    while let Some(change) = db_watcher.next().await {
        println!("Database config changed: {} = {:?}", change.key, change.new_value);
        // Reconnect database pool
    }
});

// Watch with patterns
let mut all_watcher = config.watch("").await?;  // Watch everything
let mut app_watcher = config.watch("app.*").await?;  // Watch app.* keys
```

Update configuration at runtime:

```rust
// Update individual values
config.set_value("app.debug", true.into()).await?;
config.set_value("database.max_connections", 50.into()).await?;

// Cluster-wide updates (when using distributed backends)
config.set_value_for_cluster("feature_flags.maintenance_mode", true.into()).await?;
```

## 🚩 Feature Flags

Built-in feature flag support:

```rust
// Check feature flags
if config.is_feature_enabled("new_ui")? {
    // Use new UI
}

if config.is_feature_enabled("beta_features")? {
    // Enable beta functionality
}

// Toggle feature flags
config.set_value("feature_flags.new_feature", true.into()).await?;

// Node-specific feature flags
let node_id = "node-1";
if config.get_value_for_node("feature_flags.canary_feature", node_id)?.as_bool()? {
    // This feature is only enabled for specific nodes
}
```

## 📜 History & Versioning

Track configuration changes over time:

```rust
// Get change history
let history = config.get_history("database.host", 10).await?;
for entry in history {
    println!("{:?}: {} = {} (by {})", 
        entry.timestamp, 
        "database.host",
        entry.value, 
        entry.changed_by
    );
}

// Save current configuration snapshot
config.save_to_file("snapshots/config-2023-01-01.yaml").await?;
```

## 🌐 Distributed Backends (Optional)

Enable distributed configuration synchronization:

```toml
[dependencies]
distributed-config = { version = "0.1", features = ["redis-backend"] }
# or
distributed-config = { version = "0.1", features = ["etcd-backend"] }
# or  
distributed-config = { version = "0.1", features = ["all-backends"] }
```

## 🔧 Configuration File Examples

### YAML Configuration

```yaml
# config.yaml
app:
  name: "My Application"
  version: "1.0.0"

server:
  host: "0.0.0.0"
  port: 8080
  workers: 4
  debug: false

database:
  host: "localhost"  
  port: 5432
  username: "myuser"
  password: "mypass"
  database: "mydb"
  max_connections: 10
  timeout: 30

feature_flags:
  new_ui: true
  beta_features: false
  experimental_cache: true

cache:
  ttl: 3600
  max_size: 1000
```

### JSON Configuration

```json
{
  "app": {
    "name": "My Application",
    "version": "1.0.0"
  },
  "server": {
    "host": "0.0.0.0",
    "port": 8080,
    "workers": 4,
    "debug": false
  },
  "database": {
    "host": "localhost",
    "port": 5432,
    "username": "myuser", 
    "password": "mypass",
    "database": "mydb",
    "max_connections": 10,
    "timeout": 30
  },
  "feature_flags": {
    "new_ui": true,
    "beta_features": false
  }
}
```

## 📚 Examples

Run the examples to see the library in action:

```bash
# Basic usage example
cargo run --example basic_usage

# Distributed synchronization example  
cargo run --example distributed_sync

# Feature flags example
cargo run --example feature_flags
```

## 🧪 Testing

Run the test suite:

```bash
# Run all tests
cargo test

# Run tests with logging
RUST_LOG=debug cargo test

# Run specific test module
cargo test sources::file

# Run integration tests
cargo test --test integration
```

## 🤝 Contributing

Contributions are welcome! Please read our [Contributing Guide](CONTRIBUTING.md) for details.

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Make your changes
4. Add tests for your changes
5. Ensure tests pass (`cargo test`)
6. Commit your changes (`git commit -am 'Add amazing feature'`)
7. Push to the branch (`git push origin feature/amazing-feature`)
8. Open a Pull Request

## 📄 License

This project is licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.

## 🙏 Acknowledgments

- Built with [Tokio](https://tokio.rs/) for async runtime
- Uses [serde](https://serde.rs/) for serialization  
- Configuration watching powered by [notify](https://github.com/notify-rs/notify)
- JSON Schema validation via [jsonschema](https://github.com/Stranger6667/jsonschema-rs)
- HTTP client using [reqwest](https://github.com/seanmonstar/reqwest)

---

**Happy configuring! 🎛️**
