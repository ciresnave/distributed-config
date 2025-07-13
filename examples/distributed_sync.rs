//! Distributed synchronization example for distributed-config
//!
//! This example demonstrates how to synchronize configuration across multiple nodes
//! in a distributed environment using the distributed-config library.

use distributed_config::{
    sources::{EnvSource, FileSource},
    ConfigManager, ConfigValue,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;
use tokio::time::sleep;
use tracing::info;

#[derive(Debug, Serialize, Deserialize, Clone)]
struct NodeConfig {
    node_id: String,
    cluster_name: String,
    heartbeat_interval: Duration,
    max_retries: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct ClusterConfig {
    nodes: HashMap<String, NodeConfig>,
    shared_settings: HashMap<String, String>,
    version: u64,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    distributed_config::init_tracing();

    println!("🌐 Distributed Config - Distributed Sync Example");
    println!("=================================================");

    // Simulate multiple nodes in a cluster
    let node_ids = vec!["node-1", "node-2", "node-3"];
    let mut managers = Vec::new();

    // Create configuration managers for each node
    for node_id in &node_ids {
        let mut config = ConfigManager::new();

        // Add file source with node-specific configuration
        let file_source = FileSource::new()
            .add_file("config/cluster.yaml", None)
            .add_file(format!("config/{node_id}.yaml"), Some("node"));

        config.add_source(file_source, 10);

        // Add environment source with node-specific prefix
        let env_source = EnvSource::new()
            .prefix(format!(
                "NODE_{}_",
                node_id.to_uppercase().replace("-", "_")
            ))
            .separator("__");

        config.add_source(env_source, 20);

        managers.push((node_id.to_string(), config));
    }

    // Initialize all managers
    for (node_id, config) in &mut managers {
        info!("Initializing configuration for {}", node_id);
        config.initialize().await?;
    }

    println!("\n📊 Initial Configuration State:");
    println!("==============================");

    // Display initial configuration for each node
    for (node_id, config) in &managers {
        println!("\n🔧 Node: {node_id}");

        // Try to get node-specific configuration
        if let Ok(node_config) = config.get::<NodeConfig>("node").await {
            println!("  Node ID: {}", node_config.node_id);
            println!("  Cluster: {}", node_config.cluster_name);
            println!("  Heartbeat: {:?}", node_config.heartbeat_interval);
        }

        // Display some shared configuration values
        if let Ok(version) = config.get_value("cluster.version").await {
            println!("  Cluster Version: {}", version.as_integer().unwrap_or(0));
        }
    }

    println!("\n🔄 Simulating Configuration Changes:");
    println!("===================================");

    // Simulate configuration change propagation
    let first_manager = &managers[0].1;

    // Update a shared configuration value
    println!("\n📝 Updating cluster version on {}...", managers[0].0);
    first_manager
        .set_value("cluster.version", ConfigValue::Integer(2))
        .await?;

    // In a real distributed system, this change would be propagated to other nodes
    // through the backend (Redis, etcd, etc.). For this example, we'll simulate it.
    sleep(Duration::from_millis(100)).await;

    // Simulate receiving the update on other nodes
    for (node_id, config) in &managers[1..] {
        println!("📨 {node_id} received configuration update");
        config
            .set_value("cluster.version", ConfigValue::Integer(2))
            .await?;
    }

    println!("\n🔍 Configuration After Sync:");
    println!("============================");

    // Verify all nodes have the updated configuration
    for (node_id, config) in &managers {
        if let Ok(version) = config.get_value("cluster.version").await {
            println!(
                "  {}: Cluster Version = {}",
                node_id,
                version.as_integer().unwrap_or(0)
            );
        }
    }

    println!("\n🎯 Node-Specific Configuration Examples:");
    println!("=======================================");

    // Demonstrate node-specific configuration access
    for (node_id, config) in &managers {
        // Get configuration value specific to this node
        let worker_count = match config.get_value_for_node("workers", node_id).await {
            Ok(val) => val,
            Err(_) => distributed_config::ConfigValue::Integer(4),
        };

        println!(
            "  {}: Workers = {}",
            node_id,
            worker_count.as_integer().unwrap_or(4)
        );
    }

    println!("\n🚀 Simulating Cluster-Wide Update:");
    println!("=================================");

    // Simulate a cluster-wide configuration update
    let new_heartbeat = Duration::from_secs(10);

    for (node_id, config) in &managers {
        println!("📡 Updating heartbeat interval on {node_id}");
        config
            .set_value_for_cluster(
                "node.heartbeat_interval",
                ConfigValue::Duration(new_heartbeat),
            )
            .await?;

        // Small delay to simulate network propagation
        sleep(Duration::from_millis(50)).await;
    }

    println!("\n📈 Configuration History Example:");
    println!("================================");

    // Get configuration history for one of the nodes
    let first_manager = &managers[0].1;
    if let Ok(history) = first_manager.get_history("cluster.version", 5).await {
        println!("  Recent changes to cluster.version:");
        for (i, entry) in history.iter().enumerate() {
            // Format timestamp using chrono
            let timestamp = chrono::DateTime::<chrono::Utc>::from(entry.timestamp);
            println!(
                "    {}. {} = {:?} ({})",
                i + 1,
                timestamp.format("%H:%M:%S"),
                entry.value,
                entry.changed_by
            );
        }
    }

    println!("\n🎛️  Feature Flags Across Cluster:");
    println!("================================");

    // Demonstrate feature flag management across the cluster
    for (node_id, config) in &managers {
        let feature_enabled = config
            .is_feature_enabled("new-dashboard")
            .await
            .unwrap_or(false);

        println!("  {node_id}: new-dashboard = {feature_enabled}");
    }

    println!("\n✅ Distributed Sync Example Completed!");
    println!("=====================================");
    println!("This example demonstrated:");
    println!("  • Multiple node configuration management");
    println!("  • Node-specific configuration access");
    println!("  • Cluster-wide configuration updates");
    println!("  • Configuration change propagation");
    println!("  • Configuration history tracking");
    println!("  • Feature flag management");

    Ok(())
}

/// Helper function to create sample configuration files for the example
#[allow(dead_code)]
async fn create_sample_configs() -> Result<(), Box<dyn std::error::Error>> {
    use std::fs;

    // Create config directory
    fs::create_dir_all("config")?;

    // Cluster configuration
    let cluster_config = r#"
cluster:
  name: "production-cluster"
  version: 1
  settings:
    log_level: "info"
    max_connections: 1000
    timeout: "30s"

feature_flags:
  new-dashboard: false
  advanced-analytics: true
  beta-features: false
"#;

    fs::write("config/cluster.yaml", cluster_config)?;

    // Node-specific configurations
    let nodes = [
        ("node-1", 8080, 4),
        ("node-2", 8081, 6),
        ("node-3", 8082, 8),
    ];

    for (node_id, port, workers) in &nodes {
        let node_config = format!(
            r#"
node:
  node_id: "{node_id}"
  cluster_name: "production-cluster"
  heartbeat_interval: "5s"
  max_retries: 3

server:
  port: {port}
  workers: {workers}
  
workers: {workers}
"#
        );

        fs::write(format!("config/{node_id}.yaml"), node_config)?;
    }

    println!("📁 Sample configuration files created in config/ directory");
    Ok(())
}
