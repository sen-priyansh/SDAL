// crates/network/src/transport.rs
//
// Transport-agnostic interface for network communication.
//
// The Transport trait abstracts over HTTP, SSH, SDALP, etc.
// Each implementation handles the raw byte movement while
// the protocol layer handles the logic.

use anyhow::Result;
use std::io::Read;

/// Transport-agnostic interface for sending and receiving data.
///
/// Implementations: HttpTransport (Phase 1), SshTransport (future), SdalpTransport (future)
pub trait Transport: Send + Sync {
    /// GET a resource at the given path. Returns raw bytes.
    fn get(&self, path: &str) -> Result<Vec<u8>>;

    /// POST a JSON envelope. Returns raw response bytes.
    fn post(&self, path: &str, body: Vec<u8>) -> Result<Vec<u8>>;

    /// POST a JSON envelope and a binary stream. Returns a Reader for the response stream.
    fn post_stream(
        &self,
        path: &str,
        envelope_json: Vec<u8>,
        body_stream: Box<dyn std::io::Read + Send + 'static>,
    ) -> Result<Box<dyn std::io::Read + Send>>;

    /// POST a JSON envelope and receive a binary stream.
    fn post_receive_stream(
        &self,
        path: &str,
        body: Vec<u8>,
    ) -> Result<Box<dyn std::io::Read + Send>>;
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

    fn post_stream(
        &self,
        path: &str,
        envelope_json: Vec<u8>,
        body_stream: Box<dyn std::io::Read + Send + 'static>,
    ) -> Result<Box<dyn std::io::Read + Send>> {
        let url = format!("{}{}", self.base_url, path);

        // Combine the JSON envelope and the binary stream
        // Format: [JSON Envelope] \n [Binary Stream]
        let mut req_body = envelope_json;
        req_body.push(b'\n');
        
        let reader = std::io::Cursor::new(req_body).chain(body_stream);

        let response = self
            .client
            .post(&url)
            .body(reqwest::blocking::Body::new(reader))
            .send()
            .map_err(|e| anyhow::anyhow!("HTTP POST stream failed: {}", e))?;

        if !response.status().is_success() {
            anyhow::bail!(
                "HTTP POST {} returned status {}",
                url,
                response.status()
            );
        }

        Ok(Box::new(response))
    }

    fn post_receive_stream(
        &self,
        path: &str,
        body: Vec<u8>,
    ) -> Result<Box<dyn std::io::Read + Send>> {
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

        Ok(Box::new(response))
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

        fn post_stream(
            &self,
            path: &str,
            _envelope: Vec<u8>,
            _stream: Box<dyn std::io::Read + Send + 'static>,
        ) -> Result<Box<dyn std::io::Read + Send>> {
            let data = self
                .responses
                .get(path)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("Mock: no response for POST {}", path))?;
            Ok(Box::new(std::io::Cursor::new(data)))
        }

        fn post_receive_stream(
            &self,
            path: &str,
            _body: Vec<u8>,
        ) -> Result<Box<dyn std::io::Read + Send>> {
            let data = self
                .responses
                .get(path)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("Mock: no response for POST {}", path))?;
            Ok(Box::new(std::io::Cursor::new(data)))
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
