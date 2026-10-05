use crate::managers::anonen_cloud_auth::{token_failure, token_failure_without_body, TokenFailure};
use crate::remote_asr::AsrGatewayErr;
use crate::sealed::policy::CloudSealError;

#[derive(Debug, Clone, Copy)]
pub struct CloudModelInvalidError;

impl std::fmt::Display for CloudModelInvalidError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "このモデルは提供終了しました。新しいモデルを選んでください"
        )
    }
}

impl std::error::Error for CloudModelInvalidError {}

#[derive(Debug, Clone, Copy)]
pub struct CloudAudioTooLongError;

impl std::fmt::Display for CloudAudioTooLongError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "録音が1回あたりの上限を超えています")
    }
}

impl std::error::Error for CloudAudioTooLongError {}

#[derive(Debug, Clone, Default)]
pub struct CloudCapExceededError {
    pub which: Option<String>,
    pub resets_at: Option<String>,

    pub fallback: Option<String>,

    pub suspended: bool,
}

impl std::fmt::Display for CloudCapExceededError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.suspended {
            write!(f, "クラウドの利用が一時停止されています")
        } else {
            write!(f, "利用時間の上限に達しました")
        }
    }
}

impl std::error::Error for CloudCapExceededError {}

#[derive(Debug, Clone, Copy)]
pub struct CloudSubscriptionRequiredError;

impl std::fmt::Display for CloudSubscriptionRequiredError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "有効な契約がありません")
    }
}

impl std::error::Error for CloudSubscriptionRequiredError {}

#[derive(Debug, Clone, Copy)]
pub struct CloudAuthFailedError;

impl std::fmt::Display for CloudAuthFailedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "認証に失敗しました。再度サインインしてください")
    }
}

impl std::error::Error for CloudAuthFailedError {}

#[derive(Debug, Clone)]
pub struct CloudNetworkError {
    detail: String,
}

impl std::fmt::Display for CloudNetworkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "あのねん 0 (network_error): {}", self.detail)
    }
}

impl std::error::Error for CloudNetworkError {}

#[derive(Debug, Clone)]
pub struct CloudGatewayError {
    pub http_status: u16,
    pub code: String,
    quoted_body: String,
}

impl CloudGatewayError {
    fn head(&self) -> String {
        let code = if crate::remote_asr::is_known_wire_code(&self.code)
            || crate::remote_asr::is_local_only_code(&self.code)
        {
            self.code.as_str()
        } else {
            "unknown"
        };
        format!("あのねん {} ({})", self.http_status, code)
    }

    pub fn shown(&self) -> String {
        if crate::remote_asr::is_local_only_code(&self.code) {
            format!("{}: {}", self.head(), self.quoted_body)
        } else {
            self.head()
        }
    }
}

impl std::fmt::Display for CloudGatewayError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.head(), self.quoted_body)
    }
}

impl std::error::Error for CloudGatewayError {}

#[derive(Debug)]
pub enum Terminal {
    SubscriptionRequired,

    CapExceeded(CloudCapExceededError),

    AudioTooLong,

    ModelRetired,

    AuthFailed,

    Network(CloudNetworkError),

    Gateway(CloudGatewayError),
}

pub const TOKEN_UNAVAILABLE: &str = crate::remote_asr::TOKEN_UNAVAILABLE;

pub fn token_error(message: String) -> AsrGatewayErr {
    let (http_status, code) = match token_failure(&message) {
        TokenFailure::Transport => (0, "network_error"),

        TokenFailure::SignedOut => (401, crate::remote_asr::NO_SESSION),
        TokenFailure::Other => (0, TOKEN_UNAVAILABLE),
    };
    AsrGatewayErr {
        http_status,
        code: code.to_string(),
        which: None,
        resets_at: None,
        fallback: None,
        request_id: None,

        raw_body: token_failure_without_body(&message),
    }
}

pub fn worth_one_token_refresh(err: &AsrGatewayErr) -> bool {
    err.http_status == 401 || (err.http_status == 0 && err.code == TOKEN_UNAVAILABLE)
}

pub fn classify_terminal(err: &AsrGatewayErr) -> Terminal {
    let status = err.http_status;
    let code = err.code.as_str();
    if status == 0 && code == "network_error" {
        return Terminal::Network(CloudNetworkError {
            detail: crate::utils::quoted_body(&err.raw_body),
        });
    }
    if status == 402 {
        return Terminal::SubscriptionRequired;
    }
    if status == 429 && (code == "cap_exceeded" || code == "usage_suspended") {
        return Terminal::CapExceeded(CloudCapExceededError {
            which: err.which.clone(),
            resets_at: err.resets_at.clone(),
            fallback: err.fallback.clone(),
            suspended: code == "usage_suspended",
        });
    }
    if status == 413 {
        return Terminal::AudioTooLong;
    }
    if status == 400 && (code == "unknown_provider" || code == "unknown_model") {
        return Terminal::ModelRetired;
    }
    if status == 401 {
        return Terminal::AuthFailed;
    }
    Terminal::Gateway(CloudGatewayError {
        http_status: status,
        code: err.code.clone(),
        quoted_body: crate::utils::quoted_body(&err.raw_body),
    })
}

#[derive(Debug)]
pub enum Failure<'a> {
    ModelRetired,
    AudioTooLong,
    CapExceeded(&'a CloudCapExceededError),
    SubscriptionRequired,
    AuthFailed,

    Seal(CloudSealError),

    Network,

    Other(String),
}

pub fn failure_of(err: &anyhow::Error) -> Failure<'_> {
    if err.downcast_ref::<CloudModelInvalidError>().is_some() {
        Failure::ModelRetired
    } else if err.downcast_ref::<CloudAudioTooLongError>().is_some() {
        Failure::AudioTooLong
    } else if let Some(cap) = err.downcast_ref::<CloudCapExceededError>() {
        Failure::CapExceeded(cap)
    } else if err
        .downcast_ref::<CloudSubscriptionRequiredError>()
        .is_some()
    {
        Failure::SubscriptionRequired
    } else if err.downcast_ref::<CloudAuthFailedError>().is_some() {
        Failure::AuthFailed
    } else if let Some(seal) = err.downcast_ref::<CloudSealError>() {
        Failure::Seal(*seal)
    } else if err.downcast_ref::<CloudNetworkError>().is_some() {
        Failure::Network
    } else if let Some(gateway) = err.downcast_ref::<CloudGatewayError>() {
        Failure::Other(gateway.shown())
    } else {
        Failure::Other(err.to_string())
    }
}

impl Failure<'_> {
    pub fn has_own_window_notice(&self) -> bool {
        matches!(
            self,
            Self::ModelRetired
                | Self::AudioTooLong
                | Self::CapExceeded(_)
                | Self::SubscriptionRequired
        )
    }

    pub fn window_notice_id(&self) -> Option<&'static str> {
        match self {
            Self::Seal(seal) => Some(seal.notice_id()),
            Self::AuthFailed => Some(AUTH_FAILED_NOTICE),
            _ => None,
        }
    }

    pub fn is_network(&self) -> bool {
        matches!(self, Self::Network)
    }
}

pub const AUTH_FAILED_NOTICE: &str = "auth_failed";

pub const ALREADY_NOTIFIED: &str = "already-notified";

pub const NOTICE_PREFIX: &str = "notice:";

pub fn retry_failure_reply(err: &anyhow::Error) -> String {
    let failure = failure_of(err);
    if failure.has_own_window_notice() {
        return ALREADY_NOTIFIED.to_string();
    }
    match (failure.window_notice_id(), &failure) {
        (Some(id), _) => format!("{NOTICE_PREFIX}{id}"),

        (None, Failure::Other(text)) => text.clone(),
        (None, _) => err.to_string(),
    }
}

pub struct FailureNotification {
    pub title: &'static str,
    pub body: String,
}

pub fn failure_notification(failure: &Failure<'_>, audio_saved: bool) -> FailureNotification {
    const FAILED: &str = "あのねん: 文字起こしに失敗しました";
    let recording = if audio_saved {
        "今の録音は履歴に残っています。"
    } else {
        "今の録音は残っていません。"
    };
    match failure {
        Failure::Network => FailureNotification {
            title: FAILED,
            body: format!(
                "ネットワークに接続できませんでした。接続を確認してもう一度お試しください。{recording}"
            ),
        },
        Failure::ModelRetired => FailureNotification {
            title: "あのねん: このモデルは提供終了しました",
            body: format!("アプリを開いて、新しいモデルを選んでください。{recording}"),
        },
        Failure::AudioTooLong => FailureNotification {
            title: "あのねん: 録音が1回あたりの上限を超えています",
            body: if audio_saved {
                "録音は履歴に残っています。履歴からローカルモデルで再転写できます。".to_string()
            } else {
                "この録音は残っていません。短く区切って、もう一度録音してください。".to_string()
            },
        },
        Failure::CapExceeded(cap) => FailureNotification {
            title: if cap.suspended {
                "あのねん: クラウドの利用が一時停止されています"
            } else {
                "あのねん: 利用時間の上限に達しました"
            },
            body: format!(
                "{}{}{recording}",
                if cap.suspended {
                    "再開までお待ちください。"
                } else {
                    "上限がリセットされるまでお待ちください。"
                },
                if cap.fallback.as_deref() == Some("local") {
                    "設定からローカルモデルに切り替えると続けられます。"
                } else {
                    ""
                }
            ),
        },
        Failure::SubscriptionRequired => FailureNotification {
            title: "あのねん: 契約が必要です",
            body: format!("有効な契約がありません。anonen.net でご契約ください。{recording}"),
        },
        Failure::AuthFailed => FailureNotification {
            title: FAILED,
            body: format!("{}。{recording}", CloudAuthFailedError),
        },

        Failure::Seal(seal) => FailureNotification {
            title: FAILED,
            body: format!("{seal}。{recording}"),
        },
        Failure::Other(reason) => FailureNotification {
            title: FAILED,
            body: if audio_saved {
                format!(
                    "録音は履歴に残っています。設定 → 履歴から取り直せます。（{}）",
                    crate::utils::notification_reason(reason)
                )
            } else {
                format!(
                    "今の録音は残っていません。もう一度お試しください。（{}）",
                    crate::utils::notification_reason(reason)
                )
            },
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gateway(status: u16, code: &str, body: &str) -> AsrGatewayErr {
        AsrGatewayErr {
            http_status: status,
            code: code.to_string(),
            which: None,
            resets_at: None,
            fallback: None,
            request_id: None,
            raw_body: body.to_string(),
        }
    }

    fn failure_after(err: &AsrGatewayErr) -> anyhow::Error {
        match classify_terminal(err) {
            Terminal::SubscriptionRequired => anyhow::Error::new(CloudSubscriptionRequiredError),
            Terminal::CapExceeded(cap) => anyhow::Error::new(cap),
            Terminal::AudioTooLong => anyhow::Error::new(CloudAudioTooLongError),
            Terminal::ModelRetired => anyhow::Error::new(CloudModelInvalidError),
            Terminal::AuthFailed => anyhow::Error::new(CloudAuthFailedError),
            Terminal::Network(e) => anyhow::Error::new(e),
            Terminal::Gateway(e) => anyhow::Error::new(e),
        }
    }

    #[test]
    fn a_response_is_never_a_network_failure_whatever_its_body_says() {
        for body in [
            r#"{"error":"network_error"}"#,
            r#"{"error":"upstream_error","detail":"error sending request for url (x)"}"#,
            "HTTP request failed: connection refused",
        ] {
            for status in [400, 409, 500, 502, 503] {
                for code in ["unknown", "network_error", "upstream_error"] {
                    let error = failure_after(&gateway(status, code, body));
                    let failure = failure_of(&error);
                    assert!(!failure.is_network(), "{status} {code} {body}");
                    assert!(matches!(failure, Failure::Other(_)), "{status} {code}");
                }
            }
        }
    }

    #[test]
    fn a_request_that_never_completed_is_a_network_failure() {
        let error = failure_after(&gateway(
            0,
            "network_error",
            "HTTP request failed: error sending request for url (https://example.invalid/)",
        ));
        assert!(failure_of(&error).is_network());

        assert!(error
            .to_string()
            .starts_with("あのねん 0 (network_error): "));
    }

    #[test]
    fn what_this_client_stopped_on_by_itself_is_not_a_network_failure() {
        for (code, body) in [
            (
                "client_misconfigured",
                "あのねん: モデル id が \"provider/model_name\" の形ではありません: \"x\"",
            ),
            (TOKEN_UNAVAILABLE, "token refresh failed (503): {}"),
            ("client_parse_error", "missing field"),
            ("", ""),
        ] {
            let error = failure_after(&gateway(0, code, body));
            let failure = failure_of(&error);
            assert!(!failure.is_network(), "{code}");
            assert!(matches!(failure, Failure::Other(_)), "{code}");

            let shown = if code.is_empty() { "unknown" } else { code };
            assert!(
                error
                    .to_string()
                    .starts_with(&format!("あのねん 0 ({shown}): ")),
                "{error}"
            );
        }
    }

    fn after_token_failure(message: &str) -> (AsrGatewayErr, anyhow::Error) {
        let err = token_error(message.to_string());
        let failure = failure_after(&err);
        (err, failure)
    }

    #[test]
    fn an_unreachable_auth_server_is_the_network() {
        let (err, failure) = after_token_failure(
            "token refresh failed: error sending request for url (https://x.supabase.co/auth/v1/token)",
        );
        assert_eq!((err.http_status, err.code.as_str()), (0, "network_error"));
        assert!(failure_of(&failure).is_network());
        assert!(!worth_one_token_refresh(&err));
    }

    #[test]
    fn no_session_is_a_sign_in_matter() {
        for message in [
            "Not signed in to あのねん (no refresh_token). Sign in to continue.",
            "token refresh failed (400): {\"error\":\"invalid_grant\"}",
            "token refresh failed (401): {}",
        ] {
            let (err, failure) = after_token_failure(message);
            assert_eq!(err.http_status, 401, "{message}");
            assert!(worth_one_token_refresh(&err), "{message}");
            assert!(
                matches!(failure_of(&failure), Failure::AuthFailed),
                "{message}"
            );
        }
    }

    #[test]
    fn a_failing_auth_server_does_not_tell_the_user_to_sign_in_again() {
        for message in [
            "token refresh failed (429): {\"msg\":\"rate limit\"}",
            "token refresh failed (500): {}",
            "token refresh failed (503): 認証に失敗しました。再度サインインしてください",
            "Failed to parse refresh response: expected value",
            "refresh response missing access_token",
            "keyring write failed: x",
            "token refresh lock poisoned",
            "Supabase URL is not configured",
        ] {
            let (err, failure) = after_token_failure(message);
            assert_eq!(
                (err.http_status, err.code.as_str()),
                (0, TOKEN_UNAVAILABLE),
                "{message}"
            );

            assert!(worth_one_token_refresh(&err), "{message}");
            let failure = failure_of(&failure);
            assert!(matches!(failure, Failure::Other(_)), "{message}");
            assert!(!failure.is_network(), "{message}");
            assert!(failure.window_notice_id().is_none(), "{message}");
            for saved in [true, false] {
                let body = failure_notification(&failure, saved).body;
                assert!(!body.contains("サインイン"), "{message}: {body}");
                assert!(!body.contains("ネットワーク"), "{message}: {body}");
            }
        }
    }

    #[test]
    fn what_the_auth_server_wrote_is_not_quoted_to_the_user() {
        for foreign in [
            "認証に失敗しました。再度サインインしてください",
            "<!DOCTYPE html><html><head><title>Attention Required!</title>",
            "{\"msg\":\"Database error: connection to the primary refused\"}",
        ] {
            for status in [403, 429, 500, 503] {
                let message = format!("token refresh failed ({status}): {foreign}");
                let (err, error) = after_token_failure(&message);
                assert_eq!(err.raw_body, format!("token refresh failed ({status})"));

                let reply = retry_failure_reply(&error);
                assert!(!reply.contains(foreign), "{reply}");
                assert!(reply.contains(&format!("({status})")), "{reply}");

                for saved in [true, false] {
                    let body = failure_notification(&failure_of(&error), saved).body;
                    assert!(!body.contains(foreign), "{body}");
                }
            }
        }
    }

    #[test]
    fn the_codes_made_up_for_a_token_failure_cannot_arrive_in_a_body() {
        for message in [
            "token refresh failed: error sending request",
            "Not signed in to あのねん (no refresh_token). Sign in to continue.",
            "token refresh failed (503): {}",
        ] {
            let code = token_error(message.to_string()).code;
            assert!(crate::remote_asr::is_local_only_code(&code), "{code}");
        }
    }

    #[test]
    fn only_a_401_or_a_missing_token_earns_the_one_token_refresh() {
        assert!(worth_one_token_refresh(&gateway(401, "unauthorized", "{}")));
        assert!(worth_one_token_refresh(&gateway(401, "", "")));
        for (status, code) in [
            (0, "network_error"),
            (0, "client_misconfigured"),
            (400, TOKEN_UNAVAILABLE),
            (403, "forbidden"),
            (429, "rate_limited"),
            (503, TOKEN_UNAVAILABLE),
        ] {
            assert!(
                !worth_one_token_refresh(&gateway(status, code, "{}")),
                "{status} {code}"
            );
        }
    }

    #[test]
    fn each_terminal_failure_becomes_its_own_kind() {
        let cap = AsrGatewayErr {
            which: Some("week".to_string()),
            fallback: Some("local".to_string()),
            ..gateway(429, "cap_exceeded", "{}")
        };
        match failure_of(&failure_after(&cap)) {
            Failure::CapExceeded(found) => assert_eq!(found.which.as_deref(), Some("week")),
            other => panic!("cap: {other:?}"),
        }
        assert!(matches!(
            failure_of(&failure_after(&gateway(402, "subscription_required", "{}"))),
            Failure::SubscriptionRequired
        ));
        assert!(matches!(
            failure_of(&failure_after(&gateway(413, "audio_too_long", "{}"))),
            Failure::AudioTooLong
        ));
        for code in ["unknown_model", "unknown_provider"] {
            assert!(matches!(
                failure_of(&failure_after(&gateway(400, code, "{}"))),
                Failure::ModelRetired
            ));
        }
        for code in ["unauthorized", "no_session", ""] {
            assert!(matches!(
                failure_of(&failure_after(&gateway(401, code, "whatever"))),
                Failure::AuthFailed
            ));
        }

        assert!(matches!(
            failure_of(&failure_after(&gateway(429, "rate_limited", "{}"))),
            Failure::Other(_)
        ));
        assert!(matches!(
            failure_of(&anyhow::Error::new(CloudSealError::Outdated)),
            Failure::Seal(CloudSealError::Outdated)
        ));
        assert!(matches!(
            failure_of(&anyhow::anyhow!("Whisper transcription failed: boom")),
            Failure::Other(_)
        ));
    }

    #[test]
    fn a_failure_with_its_own_event_does_not_also_get_the_generic_toast() {
        let cap = CloudCapExceededError {
            which: None,
            resets_at: None,
            fallback: None,
            ..Default::default()
        };
        for failure in [
            Failure::ModelRetired,
            Failure::AudioTooLong,
            Failure::CapExceeded(&cap),
            Failure::SubscriptionRequired,
        ] {
            assert!(failure.has_own_window_notice(), "{failure:?}");
            assert!(failure.window_notice_id().is_none(), "{failure:?}");
        }
        for failure in [
            Failure::AuthFailed,
            Failure::Seal(CloudSealError::Outdated),
            Failure::Network,
            Failure::Other("x".to_string()),
        ] {
            assert!(!failure.has_own_window_notice(), "{failure:?}");
        }
    }

    #[test]
    fn a_failure_with_a_fixed_sentence_hands_the_window_its_name() {
        for seal in CloudSealError::ALL {
            assert_eq!(
                Failure::Seal(seal).window_notice_id(),
                Some(seal.notice_id())
            );
        }
        assert_eq!(
            Failure::AuthFailed.window_notice_id(),
            Some(AUTH_FAILED_NOTICE)
        );
        assert!(Failure::Network.window_notice_id().is_none());
        assert!(Failure::Other("x".to_string()).window_notice_id().is_none());
    }

    #[test]
    fn the_window_has_a_sentence_for_every_name_it_can_be_handed() {
        let ja: serde_json::Value =
            serde_json::from_str(include_str!("../../src/i18n/locales/ja/translation.json"))
                .expect("ja/translation.json is valid JSON");
        let en: serde_json::Value =
            serde_json::from_str(include_str!("../../src/i18n/locales/en/translation.json"))
                .expect("en/translation.json is valid JSON");
        let mut ids: Vec<&str> = CloudSealError::ALL.iter().map(|s| s.notice_id()).collect();
        ids.push(AUTH_FAILED_NOTICE);
        for id in ids {
            for (lang, tree) in [("ja", &ja), ("en", &en)] {
                assert!(
                    tree["errors"]["notice"][id]
                        .as_str()
                        .is_some_and(|text| !text.is_empty()),
                    "errors.notice.{id} is missing in {lang}"
                );
            }
        }
        assert_eq!(
            ja["errors"]["notice"][AUTH_FAILED_NOTICE].as_str(),
            Some(CloudAuthFailedError.to_string().as_str())
        );
    }

    #[test]
    fn a_retry_reply_is_a_marker_a_name_or_the_error_itself() {
        assert_eq!(
            retry_failure_reply(&anyhow::Error::new(CloudAudioTooLongError)),
            ALREADY_NOTIFIED
        );
        assert_eq!(
            retry_failure_reply(&anyhow::Error::new(CloudSubscriptionRequiredError)),
            ALREADY_NOTIFIED
        );
        assert_eq!(
            retry_failure_reply(&anyhow::Error::new(CloudSealError::Outdated)),
            "notice:seal_outdated"
        );
        assert_eq!(
            retry_failure_reply(&anyhow::Error::new(CloudAuthFailedError)),
            "notice:auth_failed"
        );
        assert_eq!(
            retry_failure_reply(&anyhow::anyhow!("Recording has no audio samples")),
            "Recording has no audio samples"
        );
    }

    #[test]
    fn the_window_knows_every_name_it_can_be_handed() {
        let screen = include_str!("../../src/lib/failureNotice.ts");
        let mut ids: Vec<&str> = CloudSealError::ALL.iter().map(|s| s.notice_id()).collect();
        ids.push(AUTH_FAILED_NOTICE);
        for id in &ids {
            assert!(
                screen.contains(&format!("  {id}: \"errors.notice.{id}\",")),
                "src/lib/failureNotice.ts has no entry for {id}"
            );
        }

        assert_eq!(
            screen.matches(": \"errors.notice.").count(),
            ids.len(),
            "src/lib/failureNotice.ts lists a name this side never hands over"
        );
    }

    #[test]
    fn the_markers_are_the_ones_the_history_screen_knows() {
        let screen = include_str!("../../src/lib/failureNotice.ts");
        for (name, value) in [
            ("ALREADY_NOTIFIED", ALREADY_NOTIFIED),
            ("NOTICE_PREFIX", NOTICE_PREFIX),
        ] {
            assert!(
                screen.contains(&format!("export const {name} = \"{value}\";")),
                "src/lib/failureNotice.ts must export {name} = {value:?}"
            );
        }
    }

    #[test]
    fn the_sign_in_wall_knows_the_answer_that_means_no_subscription() {
        let screen = include_str!("../../src/lib/failureNotice.ts");
        let lookup = include_str!("managers/usage.rs");
        assert!(
            screen.contains("export const NO_ACTIVE_SUBSCRIPTION = \"no_active_subscription\";")
        );
        assert!(lookup.contains("return Err(\"no_active_subscription\".to_string());"));
    }

    fn every_failure(cap: &CloudCapExceededError) -> Vec<Failure<'_>> {
        let mut all = vec![
            Failure::ModelRetired,
            Failure::AudioTooLong,
            Failure::CapExceeded(cap),
            Failure::SubscriptionRequired,
            Failure::AuthFailed,
            Failure::Network,
            Failure::Other("あのねん 500 (unknown): boom".to_string()),
        ];
        all.extend(CloudSealError::ALL.into_iter().map(Failure::Seal));
        all
    }

    #[test]
    fn the_os_notification_does_not_tell_the_user_to_retry_what_cannot_succeed() {
        let cap = CloudCapExceededError {
            which: Some("week".to_string()),
            resets_at: None,
            fallback: Some("local".to_string()),
            ..Default::default()
        };
        for failure in every_failure(&cap) {
            let may_retry = match &failure {
                Failure::Network | Failure::Other(_) => true,
                Failure::Seal(seal) => seal.is_worth_retrying(),
                _ => false,
            };
            for saved in [true, false] {
                let body = failure_notification(&failure, saved).body;
                if !may_retry {
                    assert!(
                        !body.contains("もう一度お試しください"),
                        "{failure:?}: {body}"
                    );
                    assert!(!body.contains("取り直せます"), "{failure:?}: {body}");
                }
            }
        }

        for seal in CloudSealError::ALL {
            assert_eq!(
                seal.to_string().contains("もう一度お試しください"),
                seal.is_worth_retrying(),
                "{seal:?}"
            );
        }

        let suspended = AsrGatewayErr {
            resets_at: Some("2026-10-01T00:00:00+09:00".to_string()),
            fallback: Some("local".to_string()),
            ..gateway(429, "usage_suspended", "{}")
        };
        let Terminal::CapExceeded(cap) = classify_terminal(&suspended) else {
            panic!("usage_suspended is a pause until resets_at, not \"other\"");
        };
        assert!(cap.suspended && cap.which.is_none());
        assert_eq!(cap.resets_at.as_deref(), Some("2026-10-01T00:00:00+09:00"));
        let notice = failure_notification(&Failure::CapExceeded(&cap), true);
        assert_eq!(
            notice.title,
            "あのねん: クラウドの利用が一時停止されています"
        );
        assert!(
            notice.body.contains("再開までお待ちください"),
            "{}",
            notice.body
        );
        assert!(notice.body.contains("ローカルモデル"), "{}", notice.body);
        assert!(
            !notice.body.contains("上限") && !notice.body.contains("取り直せ"),
            "{}",
            notice.body
        );
        assert_eq!(cap.to_string(), "クラウドの利用が一時停止されています");

        let over = classify_terminal(&AsrGatewayErr {
            which: Some("month".to_string()),
            ..gateway(429, "cap_exceeded", "{}")
        });
        let Terminal::CapExceeded(cap) = over else {
            panic!()
        };
        assert!(!cap.suspended);
        assert_eq!(
            failure_notification(&Failure::CapExceeded(&cap), true).title,
            "あのねん: 利用時間の上限に達しました"
        );
    }

    #[test]
    fn every_gateway_code_keeps_its_classification_through_the_wire() {
        use crate::remote_asr::KNOWN_WIRE_CODES;

        for named in [
            "cap_exceeded",
            "usage_suspended",
            "unknown_model",
            "unknown_provider",
            "stale_enclave_key",
            "sealed_required",
            "rate_limited",
            "no_active_subscription",
            "unauthorized",
            "audio_too_long",
            "payload_too_large",
        ] {
            assert!(
                KNOWN_WIRE_CODES.contains(&named),
                "{named} is not in KNOWN_WIRE_CODES"
            );
        }
        let expected = |status: u16, code: &str| -> &'static str {
            match (status, code) {
                (402, _) => "subscription",
                (429, "cap_exceeded" | "usage_suspended") => "cap",
                (413, _) => "too_long",
                (400, "unknown_provider" | "unknown_model") => "retired",
                (401, _) => "auth",
                _ => "other",
            }
        };

        let status_of = |code: &str| -> u16 {
            match code {
                "no_active_subscription" => 402,
                "cap_exceeded" | "usage_suspended" | "rate_limited" => 429,
                "audio_too_long" | "payload_too_large" => 413,
                "unknown_provider"
                | "unknown_model"
                | "sealed_required"
                | "bad_request"
                | "invalid_sealed_request"
                | "invalid_webhook" => 400,
                "validation_error" | "invalid_nonce" => 422,
                "unauthorized" => 401,
                "stale_enclave_key" | "duplicate_request" => 409,
                "not_found" => 404,
                "method_not_allowed" => 405,
                "request_timeout" => 408,
                "internal_error" | "plan_limits_missing" | "webhook_handle_failed" => 500,
                "upstream_error" | "confirm_failed" => 502,
                _ => 503,
            }
        };
        for code in KNOWN_WIRE_CODES {
            let status = status_of(code);
            let body = format!(r#"{{"error":"{code}"}}"#);

            for parsed in [
                crate::remote_asr::parse_gateway_error(status, body.clone()),
                crate::remote_asr::read_opened_for_test(status, &body),
            ] {
                assert_eq!(parsed.code, *code, "{code} did not survive the wire");
                let kind = match classify_terminal(&parsed) {
                    Terminal::SubscriptionRequired => "subscription",
                    Terminal::CapExceeded(_) => "cap",
                    Terminal::AudioTooLong => "too_long",
                    Terminal::ModelRetired => "retired",
                    Terminal::AuthFailed => "auth",
                    Terminal::Network(_) => "network",
                    Terminal::Gateway(_) => "other",
                };
                assert_eq!(kind, expected(status, code), "{status} {code}");
            }
        }
    }

    #[test]
    fn the_os_notification_says_whether_the_recording_is_kept() {
        let cap = CloudCapExceededError {
            which: None,
            resets_at: None,
            fallback: None,
            ..Default::default()
        };
        for failure in every_failure(&cap) {
            let kept = failure_notification(&failure, true).body;
            let lost = failure_notification(&failure, false).body;
            assert!(kept.contains("履歴に残っています"), "{failure:?}: {kept}");
            assert!(!lost.contains("履歴に残っています"), "{failure:?}: {lost}");
            assert!(!lost.contains("取り直せます"), "{failure:?}: {lost}");
            assert!(!lost.contains("再転写できます"), "{failure:?}: {lost}");
            assert!(lost.contains("残っていません"), "{failure:?}: {lost}");
        }
    }

    #[test]
    fn only_an_unclassified_failure_quotes_its_reason() {
        let other = Failure::Other("Whisper transcription failed: boom".to_string());
        assert!(failure_notification(&other, true)
            .body
            .contains("Whisper transcription failed"));

        let foreign = "認証に失敗しました。再度サインインしてください";
        let error = failure_after(&gateway(503, "upstream_error", foreign));
        let failure = failure_of(&error);
        assert!(
            matches!(&failure, Failure::Other(text) if text == "あのねん 503 (upstream_error)")
        );
        for saved in [true, false] {
            let body = failure_notification(&failure, saved).body;
            assert!(!body.contains(foreign), "{body}");
            assert!(body.contains("あのねん 503 (upstream_error)"), "{body}");
        }
        assert_eq!(retry_failure_reply(&error), "あのねん 503 (upstream_error)");
        assert!(error.to_string().contains(foreign));

        for prose in [
            foreign,
            "upstream error\u{7}\n(see logs)",
            "Unauthorized",
            "sign_in_again_at_example_com",
        ] {
            let error = failure_after(&gateway(503, prose, ""));
            assert_eq!(
                retry_failure_reply(&error),
                "あのねん 503 (unknown)",
                "{prose:?}"
            );
            let body = failure_notification(&failure_of(&error), true).body;
            assert!(
                body.contains("あのねん 503 (unknown)") && !body.contains("logs"),
                "{body}"
            );
        }

        let own = failure_after(&gateway(
            0,
            "client_misconfigured",
            "あのねん: モデル id が形に合わない",
        ));
        assert_eq!(
            retry_failure_reply(&own),
            "あのねん 0 (client_misconfigured): あのねん: モデル id が形に合わない"
        );
        let unreadable = failure_after(&gateway(
            200,
            "client_parse_error",
            "response is not JSON (expected value at line 1 column 1); 12 bytes",
        ));
        assert_eq!(
            retry_failure_reply(&unreadable),
            "あのねん 200 (client_parse_error): response is not JSON (expected value at line 1 column 1); 12 bytes"
        );

        assert_eq!(
            failure_after(&gateway(
                0,
                "token_unavailable",
                "token refresh failed (503)"
            ))
            .to_string(),
            "あのねん 0 (token_unavailable): token refresh failed (503)"
        );
        assert_eq!(
            failure_after(&gateway(503, "upstream_error", foreign)).to_string(),
            format!("あのねん 503 (upstream_error): {foreign}")
        );

        let local = CloudCapExceededError {
            fallback: Some("local".to_string()),
            ..Default::default()
        };
        let none = CloudCapExceededError::default();
        assert!(failure_notification(&Failure::CapExceeded(&local), true)
            .body
            .contains("ローカルモデル"));
        assert!(!failure_notification(&Failure::CapExceeded(&none), true)
            .body
            .contains("ローカルモデル"));
    }

    #[test]
    fn a_seal_failure_is_shown_in_its_own_words() {
        for seal in CloudSealError::ALL {
            let body = failure_notification(&Failure::Seal(seal), false).body;
            assert!(body.starts_with(&seal.to_string()), "{seal:?}: {body}");
        }
    }
}
