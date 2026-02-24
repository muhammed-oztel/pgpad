use anyhow::Context;
use url::Url;

use crate::Error;

pub struct ClickHouseClient {
    pub http_client: reqwest::Client,
    pub base_url: String,
    pub database: String,
    pub username: String,
    pub password: String,
}

impl std::fmt::Debug for ClickHouseClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClickHouseClient")
            .field("base_url", &self.base_url)
            .field("database", &self.database)
            .field("username", &self.username)
            .finish()
    }
}

impl ClickHouseClient {
    /// Create a new ClickHouse client from a connection string.
    ///
    /// The connection string format is: `clickhouse://user@host:port/database`
    /// Password should be provided separately (from the keyring).
    /// Use `clickhouses://` for HTTPS connections.
    pub fn new(connection_string: &str, password: Option<&str>) -> Result<Self, Error> {
        let url = Url::parse(connection_string)
            .context("Failed to parse ClickHouse connection string")?;

        let host = url.host_str().unwrap_or("localhost");
        let port = url.port().unwrap_or(8123);
        let scheme = if url.scheme() == "clickhouses" {
            "https"
        } else {
            "http"
        };
        let base_url = format!("{}://{}:{}", scheme, host, port);

        let database = url.path().trim_start_matches('/').to_string();
        let database = if database.is_empty() {
            "default".to_string()
        } else {
            database
        };

        let username = if url.username().is_empty() {
            "default".to_string()
        } else {
            url.username().to_string()
        };

        // Use the explicitly provided password, or fall back to the one in the URL
        let password = password
            .map(|p| p.to_string())
            .or_else(|| url.password().map(|p| p.to_string()))
            .unwrap_or_default();

        let http_client = reqwest::Client::builder()
            .build()
            .context("Failed to create HTTP client")?;

        Ok(Self {
            http_client,
            base_url,
            database,
            username,
            password,
        })
    }

    /// Ping the ClickHouse server to check connectivity.
    pub async fn ping(&self) -> Result<(), Error> {
        let response = self
            .http_client
            .get(format!("{}/ping", self.base_url))
            .send()
            .await
            .context("Failed to ping ClickHouse server")?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(Error::Any(anyhow::anyhow!(
                "ClickHouse ping failed with status: {}",
                response.status()
            )))
        }
    }

    /// Execute a raw query against ClickHouse, returning the response body as text.
    pub async fn execute_raw(&self, query: &str) -> Result<String, Error> {
        let response = self
            .http_client
            .post(&self.base_url)
            .query(&[
                ("database", &self.database),
                ("user", &self.username),
                ("password", &self.password),
            ])
            .body(query.to_string())
            .send()
            .await
            .context("Failed to send query to ClickHouse")?;

        let status = response.status();
        let body = response.text().await.context("Failed to read response")?;

        if status.is_success() {
            Ok(body)
        } else {
            Err(Error::Any(anyhow::anyhow!(
                "ClickHouse query failed: {}",
                body.trim()
            )))
        }
    }
}
