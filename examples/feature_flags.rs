//! Feature flags example for distributed-config
//!
//! This example demonstrates how to use feature flags with the distributed-config library,
//! including dynamic feature flag management, A/B testing, and conditional feature rollouts.

use chrono::{DateTime, Utc};
use distributed_config::{
    sources::{EnvSource, FileSource},
    ConfigManager,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;
use tokio::time::sleep;

#[derive(Debug, Serialize, Deserialize, Clone)]
struct FeatureFlag {
    name: String,
    enabled: bool,
    rollout_percentage: f64,
    target_groups: Vec<String>,
    start_date: Option<DateTime<Utc>>,
    end_date: Option<DateTime<Utc>>,
    metadata: HashMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct User {
    id: String,
    email: String,
    groups: Vec<String>,
    experiment_bucket: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct FeatureConfig {
    flags: HashMap<String, FeatureFlag>,
    ab_tests: HashMap<String, ABTest>,
    rollout_strategy: RolloutStrategy,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct ABTest {
    name: String,
    enabled: bool,
    traffic_split: HashMap<String, f64>,
    start_date: DateTime<Utc>,
    end_date: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct RolloutStrategy {
    default_enabled: bool,
    gradual_rollout: bool,
    rollout_interval: Duration,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    distributed_config::init_tracing();

    println!("🚩 Distributed Config - Feature Flags Example");
    println!("==============================================");

    // Create configuration manager
    let mut config = ConfigManager::new();

    // Add file source for feature flags
    let file_source = FileSource::new()
        .add_file("config/features.yaml", None)
        .add_file("config/ab_tests.yaml", Some("experiments"));

    config.add_source(file_source, 10);

    // Add environment source for feature flag overrides
    let env_source = EnvSource::new().prefix("FEATURE_").separator("__");

    config.add_source(env_source, 20);

    // Initialize configuration
    config.initialize().await?;

    // Create sample configuration files
    create_sample_feature_configs().await?;

    println!("\n📊 Current Feature Flags:");
    println!("========================");

    // Display all feature flags
    if let Ok(feature_config) = config.get::<FeatureConfig>("features").await {
        for (name, flag) in &feature_config.flags {
            let status = if flag.enabled { "✅" } else { "❌" };
            println!(
                "  {} {}: {} ({}% rollout)",
                status, name, flag.enabled, flag.rollout_percentage
            );

            if !flag.target_groups.is_empty() {
                println!("    Target Groups: {:?}", flag.target_groups);
            }
        }
    }

    println!("\n🎯 Feature Flag Evaluation:");
    println!("===========================");

    // Create sample users for testing
    let users = vec![
        User {
            id: "user-1".to_string(),
            email: "alice@example.com".to_string(),
            groups: vec!["beta-testers".to_string()],
            experiment_bucket: 15,
        },
        User {
            id: "user-2".to_string(),
            email: "bob@example.com".to_string(),
            groups: vec!["premium".to_string()],
            experiment_bucket: 75,
        },
        User {
            id: "user-3".to_string(),
            email: "charlie@example.com".to_string(),
            groups: vec!["standard".to_string()],
            experiment_bucket: 45,
        },
    ];

    // Test feature flag evaluation for each user
    for user in &users {
        println!("\n👤 User: {} ({})", user.id, user.email);

        // Check basic feature flags
        let new_ui_enabled = evaluate_feature_flag(&config, "new-ui", user).await?;
        let dark_mode_enabled = evaluate_feature_flag(&config, "dark-mode", user).await?;
        let premium_features_enabled =
            evaluate_feature_flag(&config, "premium-features", user).await?;

        println!("  🎨 New UI: {}", if new_ui_enabled { "✅" } else { "❌" });
        println!(
            "  🌙 Dark Mode: {}",
            if dark_mode_enabled { "✅" } else { "❌" }
        );
        println!(
            "  💎 Premium Features: {}",
            if premium_features_enabled {
                "✅"
            } else {
                "❌"
            }
        );
    }

    println!("\n🧪 A/B Testing:");
    println!("===============");

    // Demonstrate A/B testing
    for user in &users {
        let variant = get_ab_test_variant(&config, "homepage-redesign", user).await?;
        println!("  👤 {}: Homepage Variant = {}", user.id, variant);
    }

    println!("\n🔄 Dynamic Feature Flag Updates:");
    println!("================================");

    // Simulate dynamic feature flag updates
    println!("📝 Enabling dark-mode feature flag...");
    config
        .set_value(
            "features.flags.dark-mode.enabled",
            distributed_config::ConfigValue::Bool(true),
        )
        .await?;

    // Small delay to simulate propagation
    sleep(Duration::from_millis(100)).await;

    println!("🔍 Re-evaluating feature flags after update:");
    for user in &users {
        let dark_mode_enabled = evaluate_feature_flag(&config, "dark-mode", user).await?;
        println!(
            "  👤 {}: Dark Mode = {}",
            user.id,
            if dark_mode_enabled { "✅" } else { "❌" }
        );
    }

    println!("\n📈 Feature Flag Analytics:");
    println!("==========================");

    // Simulate feature flag usage analytics
    let mut usage_stats = HashMap::new();

    for user in &users {
        for feature in &["new-ui", "dark-mode", "premium-features"] {
            let enabled = evaluate_feature_flag(&config, feature, user).await?;
            let counter = usage_stats.entry(feature.to_string()).or_insert(0);
            if enabled {
                *counter += 1;
            }
        }
    }

    println!("Feature Flag Usage (out of {} users):", users.len());
    for (feature, count) in usage_stats {
        let percentage = (count as f64 / users.len() as f64) * 100.0;
        println!("  📊 {feature}: {count} users ({percentage:.1}%)");
    }

    println!("\n🎛️  Feature Flag Management:");
    println!("============================");

    // Demonstrate feature flag management operations
    println!("📝 Creating a new feature flag...");
    let new_flag = FeatureFlag {
        name: "experimental-search".to_string(),
        enabled: false,
        rollout_percentage: 10.0,
        target_groups: vec!["beta-testers".to_string()],
        start_date: Some(Utc::now()),
        end_date: None,
        metadata: HashMap::from([
            (
                "description".to_string(),
                "New search algorithm".to_string(),
            ),
            ("owner".to_string(), "search-team".to_string()),
        ]),
    };

    // In a real implementation, this would update the configuration
    println!(
        "✅ Feature flag '{}' created with {}% rollout",
        new_flag.name, new_flag.rollout_percentage
    );

    println!("\n📊 Gradual Rollout Simulation:");
    println!("==============================");

    // Simulate gradual rollout
    let rollout_steps = vec![10.0, 25.0, 50.0, 75.0, 100.0];

    for percentage in rollout_steps {
        println!("🔄 Rolling out to {percentage}% of users...");
        config
            .set_value(
                "features.flags.new-ui.rollout_percentage",
                distributed_config::ConfigValue::Float(percentage),
            )
            .await?;

        // Simulate some time passing
        sleep(Duration::from_millis(200)).await;

        // Check how many users would see the feature
        let mut enabled_count = 0;
        for user in &users {
            if simulate_rollout_check(user, percentage) {
                enabled_count += 1;
            }
        }

        println!(
            "  📈 {} out of {} users would see the feature",
            enabled_count,
            users.len()
        );
    }

    println!("\n🎯 Targeted Feature Rollout:");
    println!("============================");

    // Demonstrate targeted rollouts
    let target_groups = vec!["beta-testers", "premium", "standard"];

    for group in target_groups {
        let users_in_group: Vec<_> = users
            .iter()
            .filter(|u| u.groups.contains(&group.to_string()))
            .collect();

        println!("👥 Group '{}': {} users", group, users_in_group.len());

        for user in users_in_group {
            let has_access = evaluate_group_access(user, group);
            println!(
                "  👤 {}: Access = {}",
                user.id,
                if has_access { "✅" } else { "❌" }
            );
        }
    }

    println!("\n✅ Feature Flags Example Completed!");
    println!("===================================");
    println!("This example demonstrated:");
    println!("  • Basic feature flag management");
    println!("  • User-based feature flag evaluation");
    println!("  • A/B testing configuration");
    println!("  • Dynamic feature flag updates");
    println!("  • Gradual rollout strategies");
    println!("  • Targeted feature rollouts");
    println!("  • Feature flag analytics");

    Ok(())
}

/// Evaluate a feature flag for a specific user
async fn evaluate_feature_flag(
    config: &ConfigManager,
    feature_name: &str,
    user: &User,
) -> Result<bool, Box<dyn std::error::Error>> {
    let flag_path = format!("features.flags.{feature_name}");

    // Check if feature flag exists and is enabled
    let enabled = config
        .get_value(&format!("{flag_path}.enabled"))
        .await?
        .as_bool()
        .unwrap_or(false);

    if !enabled {
        return Ok(false);
    }

    // Check rollout percentage
    let rollout_percentage = config
        .get_value(&format!("{flag_path}.rollout_percentage"))
        .await?
        .as_float()
        .unwrap_or(0.0);

    // Use user's experiment bucket for consistent rollout
    let user_percentage = (user.experiment_bucket as f64 / 100.0) * 100.0;
    if user_percentage > rollout_percentage {
        return Ok(false);
    }

    // Check target groups
    if let Ok(target_groups) = config
        .get_value(&format!("{flag_path}.target_groups"))
        .await
    {
        if let Ok(groups) = target_groups.as_array() {
            let has_target_group = groups.iter().any(|group| {
                if let Ok(group_str) = group.as_string() {
                    user.groups.contains(&group_str)
                } else {
                    false
                }
            });

            if !has_target_group {
                return Ok(false);
            }
        }
    }

    Ok(true)
}

/// Get A/B test variant for a user
async fn get_ab_test_variant(
    config: &ConfigManager,
    test_name: &str,
    user: &User,
) -> Result<String, Box<dyn std::error::Error>> {
    let test_path = format!("experiments.ab_tests.{test_name}");

    // Check if test is enabled
    let enabled = config
        .get_value(&format!("{test_path}.enabled"))
        .await?
        .as_bool()
        .unwrap_or(false);

    if !enabled {
        return Ok("control".to_string());
    }

    // Simple hash-based assignment (in real implementation, use more sophisticated logic)
    let bucket = user.experiment_bucket % 100;

    if bucket < 50 {
        Ok("control".to_string())
    } else {
        Ok("variant".to_string())
    }
}

/// Simulate rollout percentage check
fn simulate_rollout_check(user: &User, rollout_percentage: f64) -> bool {
    let user_percentage = (user.experiment_bucket as f64 / 100.0) * 100.0;
    user_percentage <= rollout_percentage
}

/// Check if user has access based on group membership
fn evaluate_group_access(user: &User, target_group: &str) -> bool {
    user.groups.contains(&target_group.to_string())
}

/// Create sample feature flag configuration files
async fn create_sample_feature_configs() -> Result<(), Box<dyn std::error::Error>> {
    use std::fs;

    // Create config directory
    fs::create_dir_all("config")?;

    // Feature flags configuration
    let features_config = r#"
features:
  flags:
    new-ui:
      name: "new-ui"
      enabled: true
      rollout_percentage: 50.0
      target_groups: ["beta-testers"]
      metadata:
        description: "New user interface design"
        owner: "ui-team"
    
    dark-mode:
      name: "dark-mode"
      enabled: false
      rollout_percentage: 25.0
      target_groups: []
      metadata:
        description: "Dark mode theme"
        owner: "design-team"
    
    premium-features:
      name: "premium-features"
      enabled: true
      rollout_percentage: 100.0
      target_groups: ["premium"]
      metadata:
        description: "Premium user features"
        owner: "product-team"
  
  rollout_strategy:
    default_enabled: false
    gradual_rollout: true
    rollout_interval: "1h"
"#;

    fs::write("config/features.yaml", features_config)?;

    // A/B tests configuration
    let ab_tests_config = r#"
experiments:
  ab_tests:
    homepage-redesign:
      name: "homepage-redesign"
      enabled: true
      traffic_split:
        control: 50.0
        variant: 50.0
      start_date: "2025-01-01T00:00:00Z"
      end_date: "2025-03-01T00:00:00Z"
    
    checkout-flow:
      name: "checkout-flow"
      enabled: false
      traffic_split:
        control: 70.0
        variant: 30.0
      start_date: "2025-02-01T00:00:00Z"
      end_date: "2025-04-01T00:00:00Z"
"#;

    fs::write("config/ab_tests.yaml", ab_tests_config)?;

    println!("📁 Sample feature flag configuration files created in config/ directory");
    Ok(())
}
