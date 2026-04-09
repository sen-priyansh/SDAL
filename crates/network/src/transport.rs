// crates/network/src/transport.rs
//
// Transport-agnostic interface for network communication.
//
// The Transport trait abstracts over HTTP, SSH, SDALP, etc.
// Each implementation handles the raw byte movement while
// the protocol layer handles the logic.

use anyhow::Result;

/// Transport-agnostic interface for sending and receiving data.
///
/// Implementations: HttpTransport (Phase 1), SshTransport (future), SdalpTransport (future)
pub trait Transport: Send + Sync {
    /// GET a resource at the given path. Returns raw bytes.
    fn get(&self, path: &str) -> Result<Vec<u8>>;

    /// POST data to the given path. Returns raw response bytes.
    fn post(&self, path: &str, body: Vec<u8>) -> Result<Vec<u8>>;
}

/// HTTP transport using reqwest (async under the hood, blocking bridge).
pub struct HttpTransport {
    base_url: String,
    client: reqwest::blocking::Client,
}

impl HttpTransport {
    pub fn new(base_url: &str) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            client: reqwest::blocking::Client::new(),
        }
    }
}

impl Transport for HttpTransport {
    fn get(&self, path: &str) -> Result<Vec<u8>> {
        let url = format!("{}{}", self.base_url, path);
        let response = self
            .client
            .get(&url)
            .send()
            .map_err(|e| anyhow::anyhow!("HTTP GET failed: {}", e))?;

        if !response.status().is_success() {
            anyhow::bail!(
                "HTTP GET {} returned status {}",
                url,
                response.status()
            );
        }

        let bytes = response
            .bytes()
            .map_err(|e| anyhow::anyhow!("Failed to read response body: {}", e))?;

        Ok(bytes.to_vec())
    }

    fn post(&self, path: &str, body: Vec<u8>) -> Result<Vec<u8>> {
        let url = format!("{}{}", self.base_url, path);
        let response = self
            .client
            .post(&url)
            .body(body)
            .send()
            .map_err(|e| anyhow::anyhow!("HTTP POST failed: {}", e))?;

        if !response.status().is_success() {
            anyhow::bail!(
                "HTTP POST {} returned status {}",
                url,
                response.status()
            );
        }

        let bytes = response
            .bytes()
            .map_err(|e| anyhow::anyhow!("Failed to read response body: {}", e))?;

        Ok(bytes.to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A mock transport for testing protocol logic without real HTTP.
    pub struct MockTransport {
        pub responses: std::collections::HashMap<String, Vec<u8>>,
    }

    impl MockTransport {
        pub fn new() -> Self {
            Self {
                responses: std::collections::HashMap::new(),
            }
        }

        pub fn set_response(&mut self, path: &str, data: Vec<u8>) {
            self.responses.insert(path.to_string(), data);
        }
    }

    impl Transport for MockTransport {
        fn get(&self, path: &str) -> Result<Vec<u8>> {
            self.responses
                .get(path)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("Mock: no response for GET {}", path))
        }

        fn post(&self, path: &str, _body: Vec<u8>) -> Result<Vec<u8>> {
            self.responses
                .get(path)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("Mock: no response for POST {}", path))
        }
    }

    #[test]
    fn test_mock_transport() {
        let mut mock = MockTransport::new();
        mock.set_response("/refs", b"{}".to_vec());

        let result = mock.get("/refs").unwrap();
        assert_eq!(result, b"{}");
    }
}
