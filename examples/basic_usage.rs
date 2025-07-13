//! Basic usage example for distributed-config
//!
//! This example demonstrates the fundamental features of the distributed-config library,
//! including loading from multiple sources, type-safe configuration access, and basic
//! configuration management.

use distributed_config::{
    sources::EnvSource, sources::FileSource, validation::SchemaValidator, ConfigManager,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;
use tokio::time::sleep;
use tracing::info;

#[derive(Debug, Serialize, Deserialize)]
struct DatabaseConfig {
    host: String,
    port: u16,
    username: String,
    password: String,
    max_connections: u32,
    timeout: Duration,
}

#[derive(Debug, Serialize, Deserialize)]
struct ServerConfig {
    host: String,
    port: u16,
    workers: u32,
    debug: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct AppConfig {
    name: String,
    version: String,
    server: ServerConfig,
    database: DatabaseConfig,
    feature_flags: HashMap<String, bool>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    distributed_config::init_tracing();

    println!("🚀 Distributed Config - Basic Usage Example");
    println!("============================================");

    // Create a temporary configuration file for this example
    let config_content = r#"
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
  password: "mypassword"
  max_connections: 10
  timeout: 30
feature_flags:
  new_ui: true
  beta_feature: false
  experimental_caching: true
"#;

    tokio::fs::write("example_config.yaml", config_content).await?;

    // Set some environment variables
    unsafe {
        std::env::set_var("APP_SERVER__DEBUG", "true");
        std::env::set_var("APP_DATABASE__HOST", "production-db.example.com");
        std::env::set_var("APP_FEATURE_FLAGS__NEW_UI", "false");
    }

    // Create configuration manager
    let mut config_manager = ConfigManager::new();

    // Add configuration sources with priorities
    // File source (priority 10)
    let file_source = FileSource::new().add_file("example_config.yaml", None);
    config_manager.add_source(file_source, 10);

    // Environment source (priority 20 - higher priority overrides file)
    let env_source = EnvSource::new().prefix("APP_").separator("__");
    config_manager.add_source(env_source, 20);

    // Add schema validation
    let validator = SchemaValidator::new()
        .add_schema_from_json(
            "server",
            distributed_config::validation::schemas::server_config(),
        )?
        .add_schema_from_json(
            "database",
            distributed_config::validation::schemas::database_config(),
        )?
        .add_schema_from_json(
            "feature_flags",
            distributed_config::validation::schemas::feature_flags(),
        )?;

    config_manager.set_validator(validator);

    // Initialize configuration
    println!("\n📂 Loading configuration from sources...");
    config_manager.initialize().await?;

    // Access typed configuration
    println!("\n🔍 Accessing typed configuration:");
    let app_config: AppConfig = config_manager.get("").await?;
    println!("  App Name: {}", app_config.name);
    println!("  App Version: {}", app_config.version);
    println!(
        "  Server: {}:{}",
        app_config.server.host, app_config.server.port
    );
    println!(
        "  Server Debug: {} (overridden by env)",
        app_config.server.debug
    );
    println!(
        "  Database: {}:{}",
        app_config.database.host, app_config.database.port
    );
    println!(
        "  Database Host: {} (overridden by env)",
        app_config.database.host
    );

    // Access individual configuration values
    println!("\n🎯 Accessing individual values:");
    let server_port = config_manager.get_value("server.port").await?;
    println!("  Server port: {}", server_port.as_integer()?);

    let db_timeout = config_manager.get_value("database.timeout").await?;
    println!(
        "  Database timeout: {} seconds",
        db_timeout.as_duration()?.as_secs()
    );

    // Check feature flags
    println!("\n🚩 Feature flags:");
    println!(
        "  New UI: {}",
        config_manager.is_feature_enabled("new_ui").await?
    );
    println!(
        "  Beta Feature: {}",
        config_manager.is_feature_enabled("beta_feature").await?
    );
    println!(
        "  Experimental Caching: {}",
        config_manager
            .is_feature_enabled("experimental_caching")
            .await?
    );

    // Update configuration at runtime
    println!("\n✏️  Updating configuration at runtime:");
    config_manager.set_value("server.workers", 8.into()).await?;
    let updated_workers = config_manager.get_value("server.workers").await?;
    println!("  Workers updated to: {}", updated_workers.as_integer()?);

    // Enable a feature flag
    config_manager
        .set_value("feature_flags.new_feature", true.into())
        .await?;
    println!(
        "  New feature flag enabled: {}",
        config_manager.is_feature_enabled("new_feature").await?
    );

    // Get configuration history
    println!("\n📜 Configuration history:");
    let history = config_manager.get_history("server.workers", 5).await?;
    for (i, entry) in history.iter().enumerate() {
        println!(
            "  {}. server.workers = {} (changed by: {})",
            i + 1,
            entry.value.as_integer().unwrap_or(0),
            entry.changed_by
        );
    }

    // Save current configuration
    println!("\n💾 Saving current configuration:");
    config_manager.save_to_file("current_config.yaml").await?;
    println!("  Configuration saved to: current_config.yaml");

    // Watch for configuration changes
    println!("\n👀 Watching for configuration changes (5 seconds):");
    let mut watcher = config_manager.watch("server").await?;

    // Spawn a task to make a change after 2 seconds
    let config_manager_clone = config_manager;
    // Spawn in a separate task but handle Send issues
    tokio::task::spawn_local(async move {
        sleep(Duration::from_secs(2)).await;
        info!("Making a configuration change...");
        let _ = config_manager_clone
            .set_value(
                "server.port",
                distributed_config::ConfigValue::Integer(9090),
            )
            .await;
    });

    // Listen for changes with timeout
    tokio::select! {
        Some(change) = watcher.next() => {
            println!("  🔔 Configuration changed!");
            println!("    Key: {}", change.key);
            println!("    New value: {:?}", change.new_value);
            println!("    Changed by: {}", change.changed_by);
        }
        _ = sleep(Duration::from_secs(5)) => {
            println!("  ⏰ Watch timeout (no changes detected)");
        }
    }

    // Cleanup
    println!("\n🧹 Cleaning up temporary files...");
    let _ = tokio::fs::remove_file("example_config.yaml").await;
    let _ = tokio::fs::remove_file("current_config.yaml").await;

    // Clean up environment variables
    unsafe {
        std::env::remove_var("APP_SERVER__DEBUG");
        std::env::remove_var("APP_DATABASE__HOST");
        std::env::remove_var("APP_FEATURE_FLAGS__NEW_UI");
    }

    println!("\n✅ Example completed successfully!");
    println!("\nThis example demonstrated:");
    println!("  • Loading configuration from multiple sources (file + environment)");
    println!("  • Source priority (environment variables override file values)");
    println!("  • Type-safe configuration access");
    println!("  • Schema validation");
    println!("  • Feature flag management");
    println!("  • Runtime configuration updates");
    println!("  • Configuration change watching");
    println!("  • Configuration history tracking");
    println!("  • Saving configuration to file");

    Ok(())
}
