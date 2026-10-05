use super::attestation::DISCOVERY_URL;
use super::keys::{AttestationSource, SourceError};
use crate::remote_asr::identify;
use log::warn;
use serde_json::Value;
use std::time::Duration;

const FETCH_TIMEOUT: Duration = Duration::from_secs(10);

pub struct HttpAttestationSource<'a> {
    client: &'a reqwest::blocking::Client,
    base_url: String,
}

impl<'a> HttpAttestationSource<'a> {
    pub fn new(client: &'a reqwest::blocking::Client, base_url: &str) -> Self {
        Self {
            client,
            base_url: base_url.trim_end_matches('/').to_string(),
        }
    }

    fn get_json(&self, url: &str) -> Result<(u16, Value), SourceError> {
        let response = self
            .client
            .get(url)
            .timeout(FETCH_TIMEOUT)
            .send()
            .map_err(|e| SourceError::Failed(format!("request failed: {}", e)))?;
        let status = response.status().as_u16();
        let body = response
            .text()
            .map_err(|e| SourceError::Failed(format!("cannot read body: {}", e)))?;
        let parsed = serde_json::from_str(&body)
            .map_err(|e| SourceError::Failed(format!("cannot parse body: {}", e)))?;
        Ok((status, parsed))
    }
}

impl AttestationSource for HttpAttestationSource<'_> {
    fn attestation(&self, nonce: &str) -> Result<Value, SourceError> {
        let url = format!("{}/v1/attestation?nonce={}", self.base_url, nonce);

        let response = identify(self.client.get(&url))
            .timeout(FETCH_TIMEOUT)
            .send()
            .map_err(|e| SourceError::Failed(format!("request failed: {}", e)))?;
        let status = response.status();

        if status.as_u16() == 501 || status.as_u16() == 404 {
            warn!(
                "[sealed] {}/v1/attestation answered {} — this host does not seal",
                self.base_url,
                status.as_u16()
            );
            return Err(SourceError::Unsupported);
        }
        if !status.is_success() {
            return Err(SourceError::Failed(format!(
                "HTTP {} from {}/v1/attestation",
                status.as_u16(),
                self.base_url
            )));
        }

        let body = response
            .text()
            .map_err(|e| SourceError::Failed(format!("cannot read body: {}", e)))?;
        serde_json::from_str(&body).map_err(|_| SourceError::Failed("malformed body".to_string()))
    }

    fn jwks(&self) -> Result<Vec<Value>, SourceError> {
        let (status, discovery) = self.get_json(DISCOVERY_URL)?;
        if status != 200 {
            return Err(SourceError::Failed(format!("discovery HTTP {}", status)));
        }
        let jwks_uri = discovery
            .get("jwks_uri")
            .and_then(Value::as_str)
            .ok_or_else(|| SourceError::Failed("discovery has no jwks_uri".to_string()))?;

        let (status, jwks) = self.get_json(jwks_uri)?;
        if status != 200 {
            return Err(SourceError::Failed(format!("jwks HTTP {}", status)));
        }
        jwks.get("keys")
            .and_then(Value::as_array)
            .cloned()
            .ok_or_else(|| SourceError::Failed("jwks has no keys".to_string()))
    }
}
