//! Remote configuration source for loading from HTTP endpoints

use crate::error::{ConfigError, Result};
use crate::sources::ConfigSource;
use crate::value::ConfigValue;
use async_trait::async_trait;
use reqwest::Client;
use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::interval;
use tracing::{debug, error, info, warn};

/// Configuration source that loads from a remote HTTP endpoint
pub struct RemoteSource {
    endpoint: String,
    #[allow(dead_code)] // Will be used when implementing actual remote functionality
    client: Client,
    auth_token: Option<String>,
    poll_interval: Duration,
    timeout: Duration,
    headers: HashMap<String, String>,
    name: String,
}

impl RemoteSource {
    /// Create a new remote source
    pub fn new() -> Self {
        Self {
            endpoint: String::new(),
            client: Client::new(),
            auth_token: None,
            poll_interval: Duration::from_secs(30),
            timeout: Duration::from_secs(10),
            headers: HashMap::new(),
            name: "remote".to_string(),
        }
    }

    /// Set the remote endpoint URL
    pub fn endpoint<S: Into<String>>(mut self, endpoint: S) -> Self {
        self.endpoint = endpoint.into();
        self
    }

    /// Set the authentication token
    pub fn auth_token<S: Into<String>>(mut self, token: S) -> Self {
        self.auth_token = Some(token.into());
        self
    }

    /// Set the polling interval for watching changes
    pub fn poll_interval(mut self, interval: Duration) -> Self {
        self.poll_interval = interval;
        self
    }

    /// Set the request timeout
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Add a custom header
    pub fn header<K: Into<String>, V: Into<String>>(mut self, key: K, value: V) -> Self {
        self.headers.insert(key.into(), value.into());
        self
    }

    /// Set the name of this source
    pub fn with_name<S: Into<String>>(mut self, name: S) -> Self {
        self.name = name.into();
        self
    }

    /// Build the HTTP client with configured settings
    fn build_client(&self) -> Client {
        let mut client_builder = Client::builder().timeout(self.timeout);

        // Add default headers
        let mut headers = reqwest::header::HeaderMap::new();

        // Add authentication if provided
        if let Some(token) = &self.auth_token {
            let auth_header = format!("Bearer {token}");
            if let Ok(header_value) = reqwest::header::HeaderValue::from_str(&auth_header) {
                headers.insert(reqwest::header::AUTHORIZATION, header_value);
            }
        }

        // Add custom headers
        for (key, value) in &self.headers {
            if let (Ok(header_name), Ok(header_value)) = (
                reqwest::header::HeaderName::from_bytes(key.as_bytes()),
                reqwest::header::HeaderValue::from_str(value),
            ) {
                headers.insert(header_name, header_value);
            }
        }

        if !headers.is_empty() {
            client_builder = client_builder.default_headers(headers);
        }

        client_builder.build().unwrap_or_else(|_| Client::new())
    }

    /// Fetch configuration from the remote endpoint
    async fn fetch_config(&self) -> Result<ConfigValue> {
        if self.endpoint.is_empty() {
            return Err(ConfigError::SourceInitializationError(
                "Remote endpoint not configured".to_string(),
            ));
        }

        info!("Fetching configuration from: {}", self.endpoint);

        let client = self.build_client();
        let response = client.get(&self.endpoint).send().await?;

        if !response.status().is_success() {
            return Err(ConfigError::NetworkError(response.error_for_status().unwrap_err()));
        }

        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|ct| ct.to_str().ok())
            .unwrap_or("application/json")
            .to_string();

        let text = response.text().await?;

        if text.trim().is_empty() {
            warn!("Remote configuration endpoint returned empty response");
            return Ok(ConfigValue::Object(HashMap::new()));
        }

        // Parse based on content type
        let config_value = if content_type.contains("application/json") {
            let json_value: serde_json::Value = serde_json::from_str(&text)?;
            ConfigValue::from(json_value)
        } else if content_type.contains("application/x-yaml") || content_type.contains("text/yaml")
        {
            let yaml_value: serde_yaml::Value = serde_yaml::from_str(&text)?;
            let json_value = serde_json::to_value(yaml_value)?;
            ConfigValue::from(json_value)
        } else if content_type.contains("application/toml") {
            let toml_value: toml::Value = toml::from_str(&text)?;
            let json_value = serde_json::to_value(toml_value)?;
            ConfigValue::from(json_value)
        } else {
            // Try to parse as JSON by default
            match serde_json::from_str::<serde_json::Value>(&text) {
                Ok(json_value) => ConfigValue::from(json_value),
                Err(_) => {
                    // Fall back to treating as a single string value
                    let mut config = HashMap::new();
                    config.insert("content".to_string(), ConfigValue::String(text));
                    ConfigValue::Object(config)
                }
            }
        };

        debug!("Successfully loaded remote configuration");
        Ok(config_value)
    }
}

impl Default for RemoteSource {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ConfigSource for RemoteSource {
    async fn load(&self) -> Result<ConfigValue> {
        self.fetch_config().await
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn supports_watching(&self) -> bool {
        true
    }

    async fn start_watching(&self) -> Result<tokio::sync::mpsc::Receiver<ConfigValue>> {
        let (tx, rx) = mpsc::channel(100);

        // Clone necessary data for the background task
        let endpoint = self.endpoint.clone();
        let client = self.build_client();
        let auth_token = self.auth_token.clone();
        let poll_interval = self.poll_interval;
        let timeout = self.timeout;
        let headers = self.headers.clone();
        let source_name = self.name.clone();

        tokio::spawn(async move {
            let mut interval = interval(poll_interval);
            let mut last_config: Option<ConfigValue> = None;

            info!("Starting remote configuration watcher for: {}", endpoint);

            loop {
                interval.tick().await;

                // Create a temporary source to fetch config
                let temp_source = RemoteSource {
                    endpoint: endpoint.clone(),
                    client: client.clone(),
                    auth_token: auth_token.clone(),
                    poll_interval,
                    timeout,
                    headers: headers.clone(),
                    name: source_name.clone(),
                };

                match temp_source.fetch_config().await {
                    Ok(config) => {
                        // Check if configuration has changed
                        let has_changed = match &last_config {
                            Some(last) => {
                                // Simple comparison - in production, you might want a more sophisticated diff
                                serde_json::to_string(last).unwrap_or_default()
                                    != serde_json::to_string(&config).unwrap_or_default()
                            }
                            None => true, // First load
                        };

                        if has_changed {
                            info!("Remote configuration changed, notifying watchers");

                            if let Err(e) = tx.send(config.clone()).await {
                                error!("Failed to send config update: {}", e);
                                break;
                            }

                            last_config = Some(config);
                        }
                    }
                    Err(e) => {
                        error!("Failed to fetch remote configuration: {}", e);
                        // Continue polling despite errors
                    }
                }
            }

            info!("Remote configuration watcher stopped for: {}", endpoint);
        });

        Ok(rx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn test_remote_source_json() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/config"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "key": "value",
                "number": 42
            })))
            .mount(&mock_server)
            .await;

        let source = RemoteSource::new().endpoint(format!("{}/config", mock_server.uri()));

        let config = source.load().await.unwrap();
        assert_eq!(
            config.get_path("key").unwrap().as_string().unwrap(),
            "value"
        );
        assert_eq!(config.get_path("number").unwrap().as_integer().unwrap(), 42);
    }

    #[tokio::test]
    async fn test_remote_source_auth() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/config"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({"authenticated": true})),
            )
            .mount(&mock_server)
            .await;

        let source = RemoteSource::new()
            .endpoint(format!("{}/config", mock_server.uri()))
            .auth_token("test-token");

        let config = source.load().await.unwrap();
        assert!(
            config.get_path("authenticated").unwrap().as_bool().unwrap()
        );
    }

    #[tokio::test]
    async fn test_remote_source_error() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/config"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&mock_server)
            .await;

        let source = RemoteSource::new().endpoint(format!("{}/config", mock_server.uri()));

        let result = source.load().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_remote_source_timeout() {
        let source = RemoteSource::new()
            .endpoint("https://httpbin.org/delay/5")
            .timeout(Duration::from_millis(100));

        let result = source.load().await;
        assert!(result.is_err());
    }
}
