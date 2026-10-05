use crate::sealed::attestation::VerifiedEnclave;
#[cfg(feature = "plaintext-dev")]
use crate::sealed::policy::PlaintextPermit;
use crate::sealed::wire;
use log::{info, warn};
#[cfg(feature = "plaintext-dev")]
use reqwest::blocking::multipart;
use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE, REFERER, USER_AGENT};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use specta::Type;
use std::time::{Duration, Instant};

#[cfg(all(feature = "plaintext-dev", not(debug_assertions)))]
compile_error!(
    "the `plaintext-dev` feature must not be compiled into a release build \
     (it re-opens the plaintext upload route; drop --features plaintext-dev)"
);

pub struct AudioPayload {
    pub bytes: Vec<u8>,

    pub mime_type: &'static str,

    pub file_extension: &'static str,

    pub format_name: &'static str,
}

pub fn build_shared_client() -> Result<reqwest::blocking::Client, String> {
    let t = Instant::now();
    let mut headers = HeaderMap::new();
    headers.insert(REFERER, HeaderValue::from_static("https://anonen.net"));
    headers.insert(
        USER_AGENT,
        HeaderValue::from_static("Anonen/1.0 (+https://anonen.net)"),
    );

    let client = reqwest::blocking::Client::builder()
        .default_headers(headers)
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(120))
        .pool_max_idle_per_host(2)
        .pool_idle_timeout(Duration::from_secs(90))
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {}", e))?;
    info!(
        "[remote-asr] HTTP client created in {}ms",
        t.elapsed().as_millis()
    );
    Ok(client)
}

pub const ANONEN_CLIENT_HEADER: &str = "X-Anonen-Client";

pub const ANONEN_CLIENT_ID: &str = concat!("desktop/", env!("CARGO_PKG_VERSION"));

pub fn identify(req: reqwest::blocking::RequestBuilder) -> reqwest::blocking::RequestBuilder {
    req.header(ANONEN_CLIENT_HEADER, ANONEN_CLIENT_ID)
}

#[derive(Serialize, Deserialize, Debug, Clone, Type)]
pub struct AsrGatewayUsage {
    pub week_used_s: i64,
    pub week_cap_s: i64,
    pub week_resets_at: String,
    pub month_used_s: i64,
    pub month_cap_s: i64,
    pub month_resets_at: String,

    #[serde(default)]
    pub max_request_s: Option<i64>,

    #[serde(default)]
    pub week_cap_full_s: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct AsrGatewayOk {
    pub text: String,
    pub duration_s: i64,
    pub usage: AsrGatewayUsage,

    pub models_version: Option<String>,

    pub model: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AsrGatewayErr {
    pub http_status: u16,

    pub code: String,

    pub which: Option<String>,

    pub resets_at: Option<String>,

    pub fallback: Option<String>,

    pub request_id: Option<String>,

    pub raw_body: String,
}

impl std::fmt::Display for AsrGatewayErr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "ASR Gateway error ({}): {}",
            self.http_status,
            if self.code.is_empty() {
                &self.raw_body
            } else {
                &self.code
            }
        )
    }
}

fn default_usage_multiplier() -> f32 {
    1.0
}

#[derive(Serialize, Deserialize, Debug, Clone, Type)]
pub struct AsrGatewayModel {
    pub id: String,
    pub display_name: String,
    pub provider: String,
    pub description: String,
    #[serde(default)]
    pub accuracy_score: f32,
    #[serde(default)]
    pub speed_score: f32,
    #[serde(default)]
    pub price_per_hour: f32,

    #[serde(default = "default_usage_multiplier")]
    pub usage_multiplier: f32,

    #[serde(default)]
    pub supported_languages: Vec<String>,
    #[serde(default)]
    pub supports_translation: bool,
    #[serde(default)]
    pub provider_label: String,
    #[serde(default)]
    pub is_recommended: bool,

    #[serde(default = "default_unknown_policy")]
    pub training_use: String,

    #[serde(default = "default_unknown_policy")]
    pub retention_kind: String,

    #[serde(default)]
    pub retention_days: Option<i64>,
}

fn default_unknown_policy() -> String {
    "unknown".to_string()
}

pub fn fetch_gateway_models(
    client: &reqwest::blocking::Client,
    base_url: &str,
) -> Result<Vec<AsrGatewayModel>, String> {
    let url = format!("{}/v1/models", base_url.trim_end_matches('/'));
    let response = identify(client.get(&url))
        .send()
        .map_err(|e| format!("GET /v1/models failed: {}", e))?;
    let status = response.status();
    let body = response
        .text()
        .map_err(|e| format!("Failed to read /v1/models body: {}", e))?;
    if !status.is_success() {
        return Err(format!(
            "/v1/models failed ({}): {}",
            status,
            crate::utils::quoted_body(&body)
        ));
    }
    let parsed: Value =
        serde_json::from_str(&body).map_err(|e| format!("Failed to parse /v1/models: {}", e))?;
    let entries = parsed
        .get("models")
        .and_then(|m| m.as_array().cloned())
        .ok_or_else(|| "/v1/models response has no `models` array".to_string())?;

    let mut models = Vec::with_capacity(entries.len());
    for entry in entries {
        match serde_json::from_value::<AsrGatewayModel>(entry) {
            Ok(m) => models.push(m),
            Err(e) => log::warn!("[anonen-cloud] skipping malformed /v1/models entry: {}", e),
        }
    }
    Ok(models)
}

#[cfg(feature = "plaintext-dev")]
#[allow(clippy::too_many_arguments)]
pub(crate) fn transcribe_via_asr_gateway_v1(
    _permit: PlaintextPermit,
    client: &reqwest::blocking::Client,
    base_url: &str,
    access_token: &str,
    audio: AudioPayload,
    request_id: &str,
    model: Option<&str>,
    language: Option<&str>,
    request_timeout: Duration,
) -> Result<AsrGatewayOk, AsrGatewayErr> {
    if base_url.trim().is_empty() {
        return Err(AsrGatewayErr {
            http_status: 0,
            code: "client_misconfigured".to_string(),
            which: None,
            resets_at: None,
            fallback: None,
            request_id: None,
            raw_body: "あのねん base URL is not configured".to_string(),
        });
    }
    if access_token.trim().is_empty() {
        return Err(AsrGatewayErr {
            http_status: 401,
            code: "unauthorized".to_string(),
            which: None,
            resets_at: None,
            fallback: None,
            request_id: None,
            raw_body: "No access token; sign in to あのねん first".to_string(),
        });
    }

    let model = checked_model(model)?;

    let url = format!("{}/v1/transcribe", base_url.trim_end_matches('/'));
    let payload_bytes = audio.bytes.len();

    let t_build = Instant::now();
    let filename = format!("audio.{}", audio.file_extension);
    let file_part = multipart::Part::bytes(audio.bytes)
        .file_name(filename)
        .mime_str(audio.mime_type)
        .map_err(|e| AsrGatewayErr {
            http_status: 0,
            code: "client_misconfigured".to_string(),
            which: None,
            resets_at: None,
            fallback: None,
            request_id: None,
            raw_body: format!("Failed to build multipart part: {}", e),
        })?;
    let mut form = multipart::Form::new()
        .part("file", file_part)
        .text("request_id", request_id.to_string());
    form = form.text("model", model.to_string());
    if let Some(lang) = language {
        if !lang.is_empty() {
            form = form.text("language", lang.to_string());
        }
    }
    let build_ms = t_build.elapsed().as_millis();

    let t_send = Instant::now();
    let response = identify(client.post(&url))
        .timeout(request_timeout)
        .bearer_auth(access_token)
        .multipart(form)
        .send()
        .map_err(|e| AsrGatewayErr {
            http_status: 0,
            code: "network_error".to_string(),
            which: None,
            resets_at: None,
            fallback: None,
            request_id: None,
            raw_body: format!("HTTP request failed: {}", e),
        })?;
    let send_ms = t_send.elapsed().as_millis();

    let t_read = Instant::now();
    let status = response.status();
    let body = response.text().map_err(|e| AsrGatewayErr {
        http_status: status.as_u16(),
        code: "network_error".to_string(),
        which: None,
        resets_at: None,
        fallback: None,
        request_id: None,
        raw_body: format!("Failed to read response body: {}", e),
    })?;
    let read_ms = t_read.elapsed().as_millis();

    info!(
        "[remote-asr] asr-gateway: payload={}B build={}ms send+server={}ms read={}ms status={} req_id={}",
        payload_bytes, build_ms, send_ms, read_ms, status, request_id
    );

    if !status.is_success() {
        return Err(parse_gateway_error(status.as_u16(), body));
    }

    parse_gateway_ok(status.as_u16(), &body)
}

fn checked_model(model: Option<&str>) -> Result<&str, AsrGatewayErr> {
    let id = model.map(str::trim).unwrap_or("");
    let shaped = match id.split_once('/') {
        Some((provider, name)) => !provider.is_empty() && !name.is_empty() && !name.contains('/'),
        None => false,
    };
    if !shaped {
        return Err(gateway_client_err(
            0,
            "client_misconfigured",
            format!(
                "あのねん: モデル id が \"provider/model_name\" の形ではありません: {:?}",
                id
            ),
        ));
    }
    Ok(id)
}

fn gateway_client_err(http_status: u16, code: &str, raw_body: String) -> AsrGatewayErr {
    AsrGatewayErr {
        http_status,
        code: code.to_string(),
        which: None,
        resets_at: None,
        fallback: None,
        request_id: None,
        raw_body,
    }
}

const LOCAL_ONLY_CODES: &[&str] = &[
    "client_misconfigured",
    "client_parse_error",
    "network_error",
    "seal_failed",
    "sealed_response_missing",
    "sealed_response_unreadable",
    NO_SESSION,
    TOKEN_UNAVAILABLE,
];

pub(crate) const NO_SESSION: &str = "no_session";
pub(crate) const TOKEN_UNAVAILABLE: &str = "token_unavailable";

pub(crate) fn is_local_only_code(code: &str) -> bool {
    LOCAL_ONLY_CODES.contains(&code)
}

pub(crate) const KNOWN_WIRE_CODES: &[&str] = &[
    "attestation_unavailable",
    "audio_too_long",
    "auth_unavailable",
    "bad_request",
    "cap_exceeded",
    "confirm_failed",
    "duplicate_request",
    "internal_error",
    "invalid_nonce",
    "invalid_sealed_request",
    "invalid_webhook",
    "method_not_allowed",
    "no_active_subscription",
    "not_found",
    "payload_too_large",
    "plan_limits_missing",
    "rate_limited",
    "request_timeout",
    "sealed_required",
    "server_busy",
    "service_paused",
    "stale_enclave_key",
    "unauthorized",
    "unknown_model",
    "unknown_provider",
    "upstream_error",
    "usage_suspended",
    "validation_error",
    "webhook_handle_failed",
];

pub(crate) fn is_known_wire_code(code: &str) -> bool {
    code == "unknown" || KNOWN_WIRE_CODES.contains(&code)
}

fn code_from_wire(raw: Option<String>) -> String {
    match raw {
        Some(code) if is_local_only_code(&code) => "unknown".to_string(),
        Some(code) if !is_known_wire_code(&code) => "unknown".to_string(),
        Some(code) => code,
        None => String::new(),
    }
}

pub(crate) fn parse_gateway_error(http_status: u16, body: String) -> AsrGatewayErr {
    let parsed: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
    let field = |name: &str| {
        parsed
            .get(name)
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    };
    AsrGatewayErr {
        http_status,
        code: code_from_wire(field("error")),
        which: field("which"),
        resets_at: field("resets_at"),
        fallback: field("fallback"),
        request_id: field("request_id"),
        raw_body: body,
    }
}

fn parse_gateway_ok(http_status: u16, body: &str) -> Result<AsrGatewayOk, AsrGatewayErr> {
    let fail = |detail: String| gateway_client_err(http_status, "client_parse_error", detail);
    let size = body.len();

    let parsed: Value = serde_json::from_str(body)
        .map_err(|e| fail(format!("response is not JSON ({}); {} bytes", e, size)))?;

    let text = parsed
        .get("text")
        .and_then(|t| t.as_str())
        .ok_or_else(|| fail(format!("response has no 'text'; {} bytes", size)))?
        .to_string();
    let duration_s = parsed
        .get("duration_s")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let usage: AsrGatewayUsage = parsed
        .get("usage")
        .and_then(|u| serde_json::from_value(u.clone()).ok())
        .ok_or_else(|| fail(format!("response has no usable 'usage'; {} bytes", size)))?;

    let models_version = parsed
        .get("models_version")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());

    let model = parsed
        .get("model")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());

    Ok(AsrGatewayOk {
        text,
        duration_s,
        usage,
        models_version,
        model,
    })
}

#[allow(clippy::too_many_arguments)]
pub fn transcribe_via_asr_gateway_v1_sealed(
    client: &reqwest::blocking::Client,
    base_url: &str,
    access_token: &str,
    audio: AudioPayload,
    request_id: &str,
    model: Option<&str>,
    language: Option<&str>,
    request_timeout: Duration,
    enclave: &VerifiedEnclave,
) -> Result<AsrGatewayOk, AsrGatewayErr> {
    if base_url.trim().is_empty() {
        return Err(gateway_client_err(
            0,
            "client_misconfigured",
            "あのねん base URL is not configured".to_string(),
        ));
    }
    if access_token.trim().is_empty() {
        return Err(gateway_client_err(
            401,
            "unauthorized",
            "No access token; sign in to あのねん first".to_string(),
        ));
    }

    let model = checked_model(model)?;

    let url = format!("{}/v1/transcribe", base_url.trim_end_matches('/'));

    let t_build = Instant::now();

    let envelope = wire::seal(
        &enclave.public_key,
        request_id,
        model,
        language.filter(|l| !l.is_empty()),
        &audio.bytes,
    )
    .map_err(|e| gateway_client_err(0, "seal_failed", e.to_string()))?;
    let payload_bytes = envelope.body.len();
    let build_ms = t_build.elapsed().as_millis();

    let t_send = Instant::now();
    let response = identify(client.post(&url))
        .timeout(request_timeout)
        .bearer_auth(access_token)
        .header(CONTENT_TYPE, wire::CONTENT_TYPE)
        .body(envelope.body)
        .send()
        .map_err(|e| {
            gateway_client_err(0, "network_error", format!("HTTP request failed: {}", e))
        })?;
    let send_ms = t_send.elapsed().as_millis();

    let t_read = Instant::now();
    let status = response.status();
    let sealed_reply = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.split(';').next().unwrap_or("").trim() == wire::CONTENT_TYPE);
    let body = response
        .bytes()
        .map_err(|e| {
            gateway_client_err(
                status.as_u16(),
                "network_error",
                format!("Failed to read response body: {}", e),
            )
        })?
        .to_vec();
    let read_ms = t_read.elapsed().as_millis();

    info!(
        "[remote-asr] asr-gateway(sealed): payload={}B build={}ms send+server={}ms read={}ms \
         status={} sealed_reply={} req_id={}",
        payload_bytes, build_ms, send_ms, read_ms, status, sealed_reply, request_id
    );

    if !sealed_reply {
        let text = String::from_utf8_lossy(&body).to_string();
        if status.is_success() {
            return Err(gateway_client_err(
                status.as_u16(),
                "sealed_response_missing",
                "Gateway answered 200 without opening the seal".to_string(),
            ));
        }
        return Err(unsealed_error(status.as_u16(), text));
    }

    let opened = wire::open_response(&envelope.response_key, &body).map_err(|_| {
        gateway_client_err(status.as_u16(), "sealed_response_unreadable", String::new())
    })?;
    let opened = String::from_utf8(opened).map_err(|_| {
        gateway_client_err(status.as_u16(), "sealed_response_unreadable", String::new())
    })?;

    read_opened(status.as_u16(), &opened)
}

pub(crate) fn unsealed_error(http_status: u16, body: String) -> AsrGatewayErr {
    let mut err = parse_gateway_error(http_status, body);
    if err.code == "cap_exceeded" || err.code == "usage_suspended" {
        err.code = "unknown".to_string();
        err.which = None;
        err.resets_at = None;
        err.fallback = None;
    }
    err
}

#[cfg(test)]
pub(crate) fn read_opened_for_test(http_status: u16, opened: &str) -> AsrGatewayErr {
    read_opened(http_status, opened).expect_err("an error body")
}

fn read_opened(http_status: u16, opened: &str) -> Result<AsrGatewayOk, AsrGatewayErr> {
    let parsed: Value = serde_json::from_str(opened).unwrap_or(Value::Null);
    if parsed.get("error").and_then(Value::as_str).is_some() {
        return Err(sealed_error(http_status, &parsed));
    }
    if !(200..300).contains(&http_status) {
        warn!(
            "[remote-asr] status {} around a sealed body that is not an error; \
             reading the sealed body ({} bytes)",
            http_status,
            opened.len()
        );
    }
    parse_gateway_ok(http_status, opened)
}

fn sealed_error(http_status: u16, parsed: &Value) -> AsrGatewayErr {
    const NAMED: [&str; 5] = ["error", "which", "resets_at", "fallback", "request_id"];
    let field = |name: &str| parsed.get(name).and_then(Value::as_str).map(str::to_string);
    let named: serde_json::Map<String, Value> = NAMED
        .iter()
        .filter_map(|name| field(name).map(|value| (name.to_string(), Value::String(value))))
        .collect();
    AsrGatewayErr {
        http_status,
        code: code_from_wire(field("error")),
        which: field("which"),
        resets_at: field("resets_at"),
        fallback: field("fallback"),
        request_id: field("request_id"),
        raw_body: Value::Object(named).to_string(),
    }
}

#[cfg(test)]
mod version_consistency {

    use super::ANONEN_CLIENT_ID;
    use std::path::{Path, PathBuf};

    fn crate_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    }

    fn json_version(path: &Path) -> String {
        let text = std::fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("{} が読めない: {e}", path.display()));
        let after = text
            .split_once("\"version\"")
            .unwrap_or_else(|| panic!("{} に version が無い", path.display()))
            .1;
        let start = after.find('"').expect("version の値が始まらない") + 1;
        let rest = &after[start..];
        rest[..rest.find('"').expect("version の値が閉じない")].to_string()
    }

    fn lock_version() -> String {
        let text = std::fs::read_to_string(crate_dir().join("Cargo.lock")).expect("Cargo.lock");
        let after = text
            .split_once("name = \"anonen\"")
            .expect("Cargo.lock に anonen パッケージが無い")
            .1;
        let line = after
            .lines()
            .find(|l| l.starts_with("version = "))
            .expect("anonen の version 行が無い");
        line.trim_start_matches("version = ")
            .trim_matches('"')
            .to_string()
    }

    #[test]
    fn all_four_places_agree() {
        let announced = env!("CARGO_PKG_VERSION");
        let tauri = json_version(&crate_dir().join("tauri.conf.json"));
        let package = json_version(&crate_dir().join("../package.json"));
        let lock = lock_version();
        assert_eq!(
            (tauri.as_str(), package.as_str(), lock.as_str()),
            (announced, announced, announced),
            "版が食い違っている（Cargo.toml={announced} / tauri.conf.json={tauri} / \
             package.json={package} / Cargo.lock={lock}）。4 か所すべて揃えること"
        );
    }

    #[test]
    fn the_announced_version_is_what_goes_in_the_header() {
        assert_eq!(
            ANONEN_CLIENT_ID,
            format!("desktop/{}", env!("CARGO_PKG_VERSION"))
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SPOKEN: &str = "来週の水曜に歯医者の予約を入れておいて";

    fn usage_json() -> String {
        r#"{"week_used_s":10,"week_cap_s":100,"week_resets_at":"2026-08-24T00:00:00Z",
            "month_used_s":10,"month_cap_s":100,"month_resets_at":"2026-09-01T00:00:00Z"}"#
            .to_string()
    }

    #[test]
    fn a_catalog_id_is_accepted_as_is() {
        for id in [
            "openai/gpt-transcribe",
            "mistral/voxtral-mini-2602",
            "alibaba-funasr/fun-asr-flash-realtime",
        ] {
            assert_eq!(id, checked_model(Some(id)).expect("catalog id"));
        }

        assert_eq!(
            "openai/whisper-1",
            checked_model(Some(" openai/whisper-1 ")).unwrap()
        );
    }

    #[test]
    fn the_shapes_the_gateway_refuses_do_not_leave_the_machine() {
        for id in [
            None,
            Some(""),
            Some("   "),
            Some("auto"),
            Some("qwen3-asr-flash"),
        ] {
            let err = checked_model(id).expect_err("must not reach the gateway");
            assert_eq!("client_misconfigured", err.code);
            assert_eq!(0, err.http_status);
        }

        for id in ["/voxtral-mini-2602", "mistral/", "/", "a/b/c"] {
            assert!(
                checked_model(Some(id)).is_err(),
                "must be refused: {:?}",
                id
            );
        }
    }

    #[test]
    fn the_refusal_says_which_value_was_wrong() {
        let err = checked_model(Some("auto")).expect_err("refused");
        assert!(
            err.raw_body.contains("auto"),
            "the operator cannot fix what the message does not name: {}",
            err.raw_body
        );
    }

    #[test]
    fn a_well_formed_success_body_parses() {
        let body = format!(
            r#"{{"text":"{}","duration_s":7,"usage":{},"models_version":"v3","model":"mistral/voxtral-mini-2602"}}"#,
            SPOKEN,
            usage_json()
        );
        let ok = parse_gateway_ok(200, &body).expect("should parse");
        assert_eq!(SPOKEN, ok.text);
        assert_eq!(7, ok.duration_s);
        assert_eq!(Some("v3".to_string()), ok.models_version);

        assert_eq!(Some("mistral/voxtral-mini-2602".to_string()), ok.model);

        assert_eq!(None, ok.usage.max_request_s);
    }

    #[test]
    fn a_success_body_that_cannot_be_used_does_not_quote_the_transcript() {
        for body in [
            format!(r#"{{"text":"{}","duration_s":7}}"#, SPOKEN),
            format!(r#"{{"text":"{}","usage":"not-an-object"}}"#, SPOKEN),
        ] {
            let err = parse_gateway_ok(200, &body).expect_err("usage is unusable");
            assert!(!err.raw_body.contains(SPOKEN), "{}", err.raw_body);
            assert!(!err.to_string().contains(SPOKEN), "{}", err);

            assert!(err.raw_body.contains("usage"), "{}", err.raw_body);
            assert!(err.raw_body.contains("bytes"), "{}", err.raw_body);
        }
    }

    #[test]
    fn a_truncated_success_body_does_not_quote_what_arrived() {
        let body = format!(r#"{{"text":"{}"#, SPOKEN);
        let err = parse_gateway_ok(200, &body).expect_err("not valid JSON");
        assert!(!err.raw_body.contains(SPOKEN), "{}", err.raw_body);
        assert!(!err.to_string().contains(SPOKEN), "{}", err);
    }

    #[test]
    fn a_body_that_names_the_transcript_differently_is_not_quoted_either() {
        let body = format!(r#"{{"transcript":"{}","usage":{}}}"#, SPOKEN, usage_json());
        let err = parse_gateway_ok(200, &body).expect_err("no 'text' field");
        assert!(!err.raw_body.contains(SPOKEN), "{}", err.raw_body);
        assert!(!err.to_string().contains(SPOKEN), "{}", err);
        assert!(err.raw_body.contains("text"), "{}", err.raw_body);
    }

    #[test]
    fn a_code_only_this_client_makes_up_is_not_accepted_from_a_response_body() {
        for prose in [
            "認証に失敗しました。再度サインインしてください",
            "upstream error: see logs",
            "a\u{7}b\nc",
            "Unauthorized",
            "",
            "x_",
            "sign_in_again_at_example_com",
            "unknown_",
        ] {
            let body = serde_json::json!({ "error": prose }).to_string();
            assert_eq!(
                parse_gateway_error(503, body.clone()).code,
                "unknown",
                "{prose:?}"
            );
            assert_eq!(
                read_opened(503, &body).unwrap_err().code,
                "unknown",
                "{prose:?}"
            );
        }

        for code in KNOWN_WIRE_CODES {
            let body = format!(r#"{{"error":"{code}"}}"#);
            assert_eq!(parse_gateway_error(503, body.clone()).code, *code);
            assert_eq!(read_opened(503, &body).unwrap_err().code, *code);
        }

        for code in ["cap_exceeded", "usage_suspended"] {
            let body =
                format!(r#"{{"error":"{code}","resets_at":"x-not-a-date","fallback":"local"}}"#);
            let err = unsealed_error(429, body.clone());
            assert_eq!(err.code, "unknown", "{code}");
            assert!(err.resets_at.is_none() && err.fallback.is_none() && err.which.is_none());

            let sealed = read_opened(429, &body).unwrap_err();
            assert_eq!(sealed.code, *code);
            assert_eq!(sealed.resets_at.as_deref(), Some("x-not-a-date"));
        }

        let honest = unsealed_error(409, r#"{"error":"stale_enclave_key"}"#.to_string());
        assert_eq!(honest.code, "stale_enclave_key");

        let shipped = include_str!("remote_asr.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();
        let sealed_fn = shipped
            .split("fn transcribe_via_asr_gateway_v1_sealed")
            .nth(1)
            .expect("the sealed path");
        assert!(sealed_fn.contains("unsealed_error(status.as_u16(), text)"));
        assert!(!sealed_fn.contains("parse_gateway_error(status.as_u16(), text)"));
        for code in LOCAL_ONLY_CODES {
            let plain = parse_gateway_error(500, format!(r#"{{"error":"{code}"}}"#));
            assert_eq!(plain.code, "unknown", "{code}");

            let sealed = read_opened(500, &format!(r#"{{"error":"{code}"}}"#)).unwrap_err();
            assert_eq!(sealed.code, "unknown", "{code}");
        }

        for code in [
            "cap_exceeded",
            "stale_enclave_key",
            "sealed_required",
            "rate_limited",
            "unauthorized",
        ] {
            let plain = parse_gateway_error(400, format!(r#"{{"error":"{code}"}}"#));
            assert_eq!(plain.code, code);
        }
        assert_eq!(parse_gateway_error(502, "<html>".to_string()).code, "");
    }

    #[test]
    fn every_code_made_up_here_is_listed() {
        let source = include_str!("remote_asr.rs");

        let shipped = source.split("#[cfg(test)]").next().unwrap();
        assert!(
            shipped.len() < source.len() && shipped.contains("fn parse_gateway_error"),
            "the scan no longer finds where the tests begin"
        );
        let literal_after = |text: &str| -> Option<String> {
            let start = text.find('"')? + 1;
            let end = start + text[start..].find('"')?;
            Some(text[start..end].to_string())
        };
        let mut found: Vec<String> = Vec::new();
        for (at, _) in shipped.match_indices("gateway_client_err(") {
            let call = &shipped[at..];
            if call.starts_with("gateway_client_err(http_status: u16") {
                continue;
            }

            found.push(literal_after(call).expect("a code literal follows"));
        }
        for (at, _) in shipped.match_indices("code: \"") {
            found.push(literal_after(&shipped[at..]).expect("a code literal follows"));
        }
        found.sort();
        found.dedup();
        assert!(
            found.len() >= 6,
            "the scan no longer matches the source: {found:?}"
        );

        let unlisted: Vec<&String> = found
            .iter()
            .filter(|code| !LOCAL_ONLY_CODES.contains(&code.as_str()) && *code != "unauthorized")
            .collect();
        assert!(unlisted.is_empty(), "not in LOCAL_ONLY_CODES: {unlisted:?}");

        let unused: Vec<&&str> = LOCAL_ONLY_CODES
            .iter()
            .filter(|code| ![NO_SESSION, TOKEN_UNAVAILABLE].contains(*code))
            .filter(|code| !found.iter().any(|f| f == **code))
            .collect();
        assert!(unused.is_empty(), "listed but never made up: {unused:?}");
    }

    #[test]
    fn a_sealed_result_under_an_error_status_is_never_quoted_as_an_error() {
        let opened = format!(
            r#"{{"text":"{}","duration_s":7,"usage":{}}}"#,
            SPOKEN,
            usage_json()
        );
        for status in [400, 401, 402, 409, 413, 429, 500, 502, 503] {
            match read_opened(status, &opened) {
                Ok(ok) => assert_eq!(SPOKEN, ok.text),
                Err(err) => panic!("status {status}: {err}"),
            }
        }
    }

    #[test]
    fn a_sealed_body_that_cannot_be_read_is_not_quoted_under_any_status() {
        for opened in [
            format!(r#"{{"text":"{}","duration_s":7}}"#, SPOKEN),
            format!(r#"{{"text":"{}"#, SPOKEN),
            format!(r#"{{"transcript":"{}"}}"#, SPOKEN),
        ] {
            for status in [200, 500] {
                let err = read_opened(status, &opened).expect_err("unusable");
                assert!(!err.raw_body.contains(SPOKEN), "{}", err.raw_body);
                assert!(!err.to_string().contains(SPOKEN), "{}", err);
            }
        }
    }

    #[test]
    fn a_sealed_error_keeps_its_named_fields_and_nothing_else() {
        let opened = format!(
            r#"{{"error":"cap_exceeded","which":"week","resets_at":"2026-08-24T00:00:00Z","echo":"{}"}}"#,
            SPOKEN
        );
        let err = read_opened(429, &opened).expect_err("the seal says error");
        assert_eq!(429, err.http_status);
        assert_eq!("cap_exceeded", err.code);
        assert_eq!(Some("week".to_string()), err.which);
        assert_eq!(Some("2026-08-24T00:00:00Z".to_string()), err.resets_at);
        assert!(err.raw_body.contains("cap_exceeded"), "{}", err.raw_body);
        assert!(!err.raw_body.contains(SPOKEN), "{}", err.raw_body);
        assert!(!err.to_string().contains(SPOKEN), "{}", err);
    }

    #[test]
    fn a_sealed_error_under_a_success_status_is_still_an_error() {
        let err = read_opened(200, r#"{"error":"cap_exceeded","which":"month"}"#)
            .expect_err("the seal says error");
        assert_eq!("cap_exceeded", err.code);
    }

    #[test]
    fn an_error_body_is_still_quoted() {
        let err = parse_gateway_error(
            429,
            r#"{"error":"cap_exceeded","which":"week","resets_at":"2026-08-24T00:00:00Z"}"#
                .to_string(),
        );
        assert_eq!("cap_exceeded", err.code);
        assert_eq!(Some("week".to_string()), err.which);
        assert!(err.raw_body.contains("cap_exceeded"));
    }
}
