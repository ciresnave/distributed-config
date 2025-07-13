//! Simple working example for distributed-config

use distributed_config::{sources::FileSource, ConfigManager};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Distributed Config - Simple Test");
    println!("===================================");

    // Create a configuration manager
    let mut config_manager = ConfigManager::new();

    // Add a simple file source
    let file_source = FileSource::new().add_file("simple_config.yaml", None);
    config_manager.add_source(file_source, 10);

    // Initialize configuration
    config_manager.initialize().await?;

    // Test basic functionality
    println!("✅ Configuration manager initialized successfully!");

    // Try to get a simple value
    match config_manager.get_value("test.value").await {
        Ok(value) => println!("  📄 Found test.value: {value:?}"),
        Err(_) => println!("  ⚠️  test.value not found (this is expected)"),
    }

    // Test setting a value
    config_manager
        .set_value(
            "runtime.test",
            distributed_config::ConfigValue::String("hello".to_string()),
        )
        .await?;

    // Test getting the value back
    let value = config_manager.get_value("runtime.test").await?;
    println!("  ✅ Set and retrieved runtime.test: {value:?}");

    // Test feature flag (should return false for non-existent flag)
    let feature_enabled = config_manager.is_feature_enabled("test_feature").await?;
    println!("  🚩 test_feature enabled: {feature_enabled}");

    println!("\n✅ Simple test completed successfully!");

    Ok(())
}
