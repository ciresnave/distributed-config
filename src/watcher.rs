//! Configuration change watching and notification system

use crate::value::ConfigValue;
use serde::{Deserialize, Serialize};
use std::time::SystemTime;
use tokio::sync::broadcast;
use tracing::debug;

/// Represents a configuration change event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigChange {
    /// The configuration key that changed
    pub key: String,
    /// The previous value (None if the key was newly created)
    pub old_value: Option<ConfigValue>,
    /// The new value
    pub new_value: ConfigValue,
    /// When the change occurred
    pub timestamp: SystemTime,
    /// Who or what caused the change
    pub changed_by: String,
}

/// Configuration watcher that can be used to monitor changes to specific keys
pub struct ConfigWatcher {
    /// Pattern to match against configuration keys
    key_pattern: String,
    /// Receiver for configuration change events
    receiver: broadcast::Receiver<ConfigChange>,
}

impl ConfigWatcher {
    /// Create a new configuration watcher
    pub fn new(key_pattern: String, receiver: broadcast::Receiver<ConfigChange>) -> Self {
        Self {
            key_pattern,
            receiver,
        }
    }

    /// Get the next configuration change that matches this watcher's pattern
    pub async fn next(&mut self) -> Option<ConfigChange> {
        loop {
            match self.receiver.recv().await {
                Ok(change) => {
                    if self.matches_pattern(&change.key) {
                        debug!(
                            "Configuration change matched pattern '{}': {}",
                            self.key_pattern, change.key
                        );
                        return Some(change);
                    }
                    // Continue waiting if the change doesn't match our pattern
                }
                Err(broadcast::error::RecvError::Closed) => {
                    debug!("Configuration change channel closed");
                    return None;
                }
                Err(broadcast::error::RecvError::Lagged(_)) => {
                    // We missed some messages, but continue listening
                    debug!("Configuration watcher lagged, continuing...");
                    continue;
                }
            }
        }
    }

    /// Get the pattern this watcher is monitoring
    pub fn pattern(&self) -> &str {
        &self.key_pattern
    }

    /// Check if a key matches this watcher's pattern
    fn matches_pattern(&self, key: &str) -> bool {
        matches_pattern(&self.key_pattern, key)
    }
}

/// Check if a configuration key matches a pattern
fn matches_pattern(pattern: &str, key: &str) -> bool {
    // Handle exact match
    if pattern == key {
        return true;
    }

    // Handle empty pattern (matches everything)
    if pattern.is_empty() {
        return true;
    }

    // Handle wildcard patterns
    if pattern.contains('*') {
        return matches_wildcard_pattern(pattern, key);
    }

    // Handle prefix match (pattern is a parent of the key)
    if pattern.ends_with('.') {
        return key.starts_with(pattern);
    }

    // Handle parent key match (key starts with pattern + dot)
    let pattern_with_dot = format!("{pattern}.");
    if key.starts_with(&pattern_with_dot) {
        return true;
    }

    false
}

/// Check if a key matches a wildcard pattern
fn matches_wildcard_pattern(pattern: &str, key: &str) -> bool {
    // Simple glob-style matching
    let pattern_parts: Vec<&str> = pattern.split('*').collect();

    if pattern_parts.len() == 1 {
        // No wildcards, just do exact match
        return pattern == key;
    }

    let mut key_pos = 0;

    for (i, part) in pattern_parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }

        if i == 0 {
            // First part must match the beginning
            if !key[key_pos..].starts_with(part) {
                return false;
            }
            key_pos += part.len();
        } else if i == pattern_parts.len() - 1 {
            // Last part must match the end
            return key[key_pos..].ends_with(part);
        } else {
            // Middle parts
            if let Some(pos) = key[key_pos..].find(part) {
                key_pos += pos + part.len();
            } else {
                return false;
            }
        }
    }

    true
}

/// A builder for creating multiple watchers with different patterns
pub struct WatcherBuilder {
    patterns: Vec<String>,
}

impl WatcherBuilder {
    /// Create a new watcher builder
    pub fn new() -> Self {
        Self {
            patterns: Vec::new(),
        }
    }

    /// Add a pattern to watch
    pub fn watch<S: Into<String>>(mut self, pattern: S) -> Self {
        self.patterns.push(pattern.into());
        self
    }

    /// Build watchers from a broadcast receiver
    pub fn build(self, receiver: broadcast::Receiver<ConfigChange>) -> Vec<ConfigWatcher> {
        self.patterns
            .into_iter()
            .map(|pattern| ConfigWatcher::new(pattern, receiver.resubscribe()))
            .collect()
    }
}

impl Default for WatcherBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Configuration change filter for more complex matching logic
pub struct ChangeFilter {
    /// Key patterns to include
    include_patterns: Vec<String>,
    /// Key patterns to exclude
    exclude_patterns: Vec<String>,
    /// Minimum time between notifications (debouncing)
    debounce_duration: Option<std::time::Duration>,
    /// Last notification time for debouncing
    last_notification: Option<SystemTime>,
}

impl ChangeFilter {
    /// Create a new change filter
    pub fn new() -> Self {
        Self {
            include_patterns: Vec::new(),
            exclude_patterns: Vec::new(),
            debounce_duration: None,
            last_notification: None,
        }
    }

    /// Add an include pattern
    pub fn include<S: Into<String>>(mut self, pattern: S) -> Self {
        self.include_patterns.push(pattern.into());
        self
    }

    /// Add an exclude pattern
    pub fn exclude<S: Into<String>>(mut self, pattern: S) -> Self {
        self.exclude_patterns.push(pattern.into());
        self
    }

    /// Set debounce duration
    pub fn debounce(mut self, duration: std::time::Duration) -> Self {
        self.debounce_duration = Some(duration);
        self
    }

    /// Check if a change should be processed based on this filter
    pub fn should_process(&mut self, change: &ConfigChange) -> bool {
        // Check debouncing
        if let Some(debounce_duration) = self.debounce_duration {
            if let Some(last_time) = self.last_notification {
                if change
                    .timestamp
                    .duration_since(last_time)
                    .unwrap_or_default()
                    < debounce_duration
                {
                    return false;
                }
            }
        }

        // Check exclude patterns first
        for exclude_pattern in &self.exclude_patterns {
            if matches_pattern(exclude_pattern, &change.key) {
                return false;
            }
        }

        // If no include patterns specified, include everything (that wasn't excluded)
        if self.include_patterns.is_empty() {
            self.last_notification = Some(change.timestamp);
            return true;
        }

        // Check include patterns
        for include_pattern in &self.include_patterns {
            if matches_pattern(include_pattern, &change.key) {
                self.last_notification = Some(change.timestamp);
                return true;
            }
        }

        false
    }
}

impl Default for ChangeFilter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tokio::sync::broadcast;

    #[test]
    fn test_pattern_matching() {
        assert!(matches_pattern("app.database", "app.database"));
        assert!(matches_pattern("app.database", "app.database.host"));
        assert!(matches_pattern("app.", "app.database.host"));
        assert!(matches_pattern("", "anything"));
        assert!(matches_pattern("app.*", "app.database"));
        assert!(matches_pattern("app.*", "app.cache"));
        assert!(matches_pattern("*.host", "database.host"));
        assert!(matches_pattern("*.host", "cache.host"));

        assert!(!matches_pattern("app.database", "app.cache"));
        assert!(!matches_pattern("app.database.host", "app.database"));
        assert!(!matches_pattern("database", "app.database"));
    }

    #[tokio::test]
    async fn test_config_watcher() {
        let (tx, rx) = broadcast::channel(10);
        let mut watcher = ConfigWatcher::new("app.database".to_string(), rx);

        // Send a matching change
        let change = ConfigChange {
            key: "app.database.host".to_string(),
            old_value: None,
            new_value: ConfigValue::String("localhost".to_string()),
            timestamp: SystemTime::now(),
            changed_by: "test".to_string(),
        };

        tx.send(change.clone()).unwrap();

        // Should receive the change
        let received = watcher.next().await.unwrap();
        assert_eq!(received.key, "app.database.host");
    }

    #[tokio::test]
    async fn test_change_filter() {
        let mut filter = ChangeFilter::new()
            .include("app.*")
            .exclude("app.secret.*")
            .debounce(Duration::from_millis(100));

        let change1 = ConfigChange {
            key: "app.database.host".to_string(),
            old_value: None,
            new_value: ConfigValue::String("localhost".to_string()),
            timestamp: SystemTime::now(),
            changed_by: "test".to_string(),
        };

        let change2 = ConfigChange {
            key: "app.secret.key".to_string(),
            old_value: None,
            new_value: ConfigValue::String("secret".to_string()),
            timestamp: SystemTime::now(),
            changed_by: "test".to_string(),
        };

        assert!(filter.should_process(&change1));
        assert!(!filter.should_process(&change2)); // Excluded

        // Test debouncing
        let change3 = ConfigChange {
            key: "app.database.port".to_string(),
            old_value: None,
            new_value: ConfigValue::Integer(5432),
            timestamp: SystemTime::now(),
            changed_by: "test".to_string(),
        };

        assert!(!filter.should_process(&change3)); // Debounced
    }

    #[test]
    fn test_wildcard_matching() {
        assert!(matches_wildcard_pattern("app.*", "app.database"));
        assert!(matches_wildcard_pattern("app.*", "app.cache"));
        assert!(matches_wildcard_pattern("*.host", "database.host"));
        assert!(matches_wildcard_pattern("app.*.host", "app.database.host"));
        assert!(matches_wildcard_pattern("*", "anything"));

        assert!(!matches_wildcard_pattern("app.*", "database.host"));
        assert!(!matches_wildcard_pattern("*.host", "database.port"));
        assert!(!matches_wildcard_pattern("app.*.host", "app.database.port"));
    }
}
