use super::attestation::{parse_accepted_digests, VerifiedEnclave};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloudSealError {
    Unavailable,

    Outdated,

    SealFailed,

    KeyLostAfterSend,

    OutdatedAfterSend,

    ResponseBroken,

    SealRefused,

    PlaintextRefused,
}

impl CloudSealError {
    pub const ALL: [Self; 8] = [
        Self::Unavailable,
        Self::Outdated,
        Self::SealFailed,
        Self::KeyLostAfterSend,
        Self::OutdatedAfterSend,
        Self::ResponseBroken,
        Self::SealRefused,
        Self::PlaintextRefused,
    ];

    pub fn notice_id(self) -> &'static str {
        match self {
            Self::Unavailable => "seal_unavailable",
            Self::Outdated => "seal_outdated",
            Self::SealFailed => "seal_failed",
            Self::KeyLostAfterSend => "seal_key_lost",
            Self::OutdatedAfterSend => "seal_outdated_after_send",
            Self::ResponseBroken => "seal_response_broken",
            Self::SealRefused => "seal_refused",
            Self::PlaintextRefused => "plaintext_refused",
        }
    }

    pub fn is_worth_retrying(self) -> bool {
        matches!(self, Self::KeyLostAfterSend)
    }
}

impl std::fmt::Display for CloudSealError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let message = match self {
            Self::Unavailable => "安全に送れる状態を確認できませんでした（音声は送っていません）",
            Self::Outdated => {
                "このアプリでは、いまのサーバーを確認できません。アプリを最新版に更新してください（音声は送っていません）"
            }
            Self::SealFailed => "音声を保護できなかったため、送信を中止しました",
            Self::KeyLostAfterSend => {
                "サーバーに送り直せませんでした。もう一度お試しください（音声は保護して送りました）"
            }
            Self::OutdatedAfterSend => {
                "このアプリでは、いまのサーバーを確認できません。アプリを最新版に更新してください（音声は保護して送りました）"
            }
            Self::ResponseBroken => {
                "保護された応答を受け取れませんでした。時間をおいて試してください（音声は保護して送りました）"
            }
            Self::SealRefused => {
                "サーバーが、保護された送信として受け付けませんでした。時間をおいて試してください（音声は保護して送りました）"
            }
            Self::PlaintextRefused => "サーバーが、保護されていない送信を受け付けませんでした",
        };
        write!(f, "{}", message)
    }
}

impl std::error::Error for CloudSealError {}

pub fn sealing_is_required(raw: Option<&str>) -> bool {
    let raw = raw.unwrap_or_default();
    !matches!(
        raw.trim().to_ascii_lowercase().as_str(),
        "0" | "false" | "no"
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UploadPolicy {
    pub sealing_required: bool,

    pub accepted_image_digests: Vec<String>,
}

impl UploadPolicy {
    pub fn from_settings(
        sealed_required_raw: Option<&str>,
        accepted_digests_raw: Option<&str>,
    ) -> Self {
        Self {
            sealing_required: sealing_is_required(sealed_required_raw),
            accepted_image_digests: parse_accepted_digests(
                accepted_digests_raw.unwrap_or_default(),
            ),
        }
    }

    pub fn from_build() -> Self {
        let sealed = crate::managers::build_time_setting(
            option_env!("ANONEN_SEALED_REQUIRED"),
            "ANONEN_SEALED_REQUIRED",
        );
        let digests = crate::managers::build_time_setting(
            option_env!("ANONEN_ACCEPTED_IMAGE_DIGESTS"),
            "ANONEN_ACCEPTED_IMAGE_DIGESTS",
        );
        let mut policy = Self::from_settings(sealed.as_deref(), digests.as_deref());
        policy.sealing_required |= !cfg!(feature = "plaintext-dev");
        policy
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PlaintextPermit(());

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoKeyReason {
    UnknownImage,

    Other,
}

#[derive(Debug, Clone)]
pub enum UploadRoute {
    Sealed(VerifiedEnclave),
    #[cfg(feature = "plaintext-dev")]
    Plaintext(PlaintextPermit),
}

#[derive(Debug, Default)]
pub struct RecordingSealState {
    sealed_once: bool,
    stale_retried: bool,
}

impl RecordingSealState {
    pub fn sealed_once(&self) -> bool {
        self.sealed_once
    }
}

pub fn decide_upload(
    policy: &UploadPolicy,
    state: &mut RecordingSealState,
    enclave: Option<VerifiedEnclave>,
    reason: NoKeyReason,
) -> Result<UploadRoute, CloudSealError> {
    match enclave {
        Some(enclave) => {
            state.sealed_once = true;
            Ok(UploadRoute::Sealed(enclave))
        }
        None if state.sealed_once => Err(match reason {
            NoKeyReason::UnknownImage => CloudSealError::OutdatedAfterSend,
            NoKeyReason::Other => CloudSealError::KeyLostAfterSend,
        }),
        None if policy.sealing_required => Err(match reason {
            NoKeyReason::UnknownImage => CloudSealError::Outdated,
            NoKeyReason::Other => CloudSealError::Unavailable,
        }),
        None => {
            #[cfg(feature = "plaintext-dev")]
            {
                Ok(UploadRoute::Plaintext(PlaintextPermit(())))
            }
            #[cfg(not(feature = "plaintext-dev"))]
            {
                Err(CloudSealError::Unavailable)
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SealErrorAction {
    RefetchKeyAndRetry,

    Stop(CloudSealError),

    StopWithGatewayError,

    NotSealing,
}

pub fn on_gateway_error(
    state: &mut RecordingSealState,
    http_status: u16,
    code: &str,
) -> SealErrorAction {
    if http_status == 409 && code == "stale_enclave_key" && state.sealed_once {
        if state.stale_retried {
            return SealErrorAction::StopWithGatewayError;
        }
        state.stale_retried = true;
        return SealErrorAction::RefetchKeyAndRetry;
    }

    if code == "seal_failed" {
        return SealErrorAction::Stop(CloudSealError::SealFailed);
    }
    if matches!(
        code,
        "sealed_response_missing" | "sealed_response_unreadable"
    ) {
        return SealErrorAction::Stop(CloudSealError::ResponseBroken);
    }
    if http_status == 400 && code == "sealed_required" {
        return SealErrorAction::Stop(if state.sealed_once {
            CloudSealError::SealRefused
        } else {
            CloudSealError::PlaintextRefused
        });
    }
    SealErrorAction::NotSealing
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_build_carries_the_committed_accepted_list() {
        let committed: Vec<String> = include_str!("../../accepted-digests.txt")
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .map(str::to_ascii_lowercase)
            .collect();
        let baked = option_env!("ANONEN_ACCEPTED_IMAGE_DIGESTS")
            .map(parse_accepted_digests)
            .unwrap_or_default();
        assert_eq!(baked, committed);
    }

    fn enclave() -> VerifiedEnclave {
        VerifiedEnclave {
            public_key: vec![7; 32],
            key_id_hex: "00".repeat(32),
            image_digest: format!("sha256:{}", "ab".repeat(32)),
            expires_at_seconds: 0,
        }
    }

    fn required() -> UploadPolicy {
        UploadPolicy::from_settings(None, Some(&format!("sha256:{}", "ab".repeat(32))))
    }

    fn optional() -> UploadPolicy {
        UploadPolicy::from_settings(Some("0"), Some(&format!("sha256:{}", "ab".repeat(32))))
    }

    #[test]
    fn sealing_is_on_unless_explicitly_switched_off() {
        for raw in [
            None,
            Some(""),
            Some("1"),
            Some("true"),
            Some("off"),
            Some("nope"),
            Some(" "),
        ] {
            assert!(
                sealing_is_required(raw),
                "{raw:?} must not reopen plaintext"
            );
        }
        for raw in [
            Some("0"),
            Some("false"),
            Some("no"),
            Some(" No "),
            Some("FALSE"),
        ] {
            assert!(
                !sealing_is_required(raw),
                "{raw:?} is the deliberate switch"
            );
        }
    }

    #[test]
    fn missing_settings_fail_closed_end_to_end() {
        let policy = UploadPolicy::from_settings(None, None);
        assert!(policy.sealing_required);
        assert!(
            policy.accepted_image_digests.is_empty(),
            "empty accepts nothing, not everything"
        );
        let mut state = RecordingSealState::default();
        assert_eq!(
            decide_upload(&policy, &mut state, None, NoKeyReason::Other).unwrap_err(),
            CloudSealError::Unavailable
        );
    }

    #[test]
    fn required_and_no_key_refuses_before_sending() {
        let mut state = RecordingSealState::default();
        assert_eq!(
            decide_upload(&required(), &mut state, None, NoKeyReason::Other).unwrap_err(),
            CloudSealError::Unavailable
        );
        assert!(!state.sealed_once());
    }

    #[test]
    fn required_and_unknown_image_names_the_cause() {
        let mut state = RecordingSealState::default();
        assert_eq!(
            decide_upload(&required(), &mut state, None, NoKeyReason::UnknownImage).unwrap_err(),
            CloudSealError::Outdated
        );
    }

    #[cfg(not(feature = "plaintext-dev"))]
    #[test]
    fn without_the_feature_optional_and_no_key_still_refuses() {
        let mut state = RecordingSealState::default();
        assert_eq!(
            decide_upload(&optional(), &mut state, None, NoKeyReason::Other).unwrap_err(),
            CloudSealError::Unavailable
        );
    }

    #[test]
    fn a_key_is_always_used_even_when_sealing_is_optional() {
        let mut state = RecordingSealState::default();
        match decide_upload(&optional(), &mut state, Some(enclave()), NoKeyReason::Other) {
            Ok(UploadRoute::Sealed(e)) => assert_eq!(e.key_id_hex, "00".repeat(32)),
            other => panic!("a key must never be ignored: {other:?}"),
        }
        assert!(state.sealed_once());
    }

    #[cfg(feature = "plaintext-dev")]
    #[test]
    fn optional_and_no_key_yields_a_permit() {
        let mut state = RecordingSealState::default();
        match decide_upload(&optional(), &mut state, None, NoKeyReason::Other) {
            Ok(UploadRoute::Plaintext(_permit)) => {}
            other => panic!("plaintext must be allowed when the switch is off: {other:?}"),
        }
        assert!(!state.sealed_once());
    }

    #[test]
    fn after_sealing_once_there_is_no_plaintext_even_when_optional() {
        let mut state = RecordingSealState::default();
        decide_upload(&optional(), &mut state, Some(enclave()), NoKeyReason::Other).unwrap();
        assert_eq!(
            decide_upload(&optional(), &mut state, None, NoKeyReason::Other).unwrap_err(),
            CloudSealError::KeyLostAfterSend
        );
    }

    #[test]
    fn after_sealing_once_an_unknown_image_is_reported_without_saying_nothing_was_sent() {
        let mut state = RecordingSealState::default();
        decide_upload(&required(), &mut state, Some(enclave()), NoKeyReason::Other).unwrap();
        let error =
            decide_upload(&required(), &mut state, None, NoKeyReason::UnknownImage).unwrap_err();
        assert_eq!(error, CloudSealError::OutdatedAfterSend);
        assert!(!error.to_string().contains("送っていません"));
        assert!(!error.to_string().contains("もう一度お試しください"));
        assert!(error.to_string().contains("更新"));
    }

    #[test]
    fn no_error_that_can_follow_a_send_says_nothing_was_sent() {
        for reason in [NoKeyReason::Other, NoKeyReason::UnknownImage] {
            for policy in [required(), optional()] {
                let mut state = RecordingSealState::default();
                decide_upload(&policy, &mut state, Some(enclave()), NoKeyReason::Other).unwrap();
                let error = decide_upload(&policy, &mut state, None, reason).unwrap_err();
                assert!(
                    !error.to_string().contains("送っていません"),
                    "after a send: {error}"
                );
            }
        }
        for error in [
            CloudSealError::KeyLostAfterSend,
            CloudSealError::OutdatedAfterSend,
            CloudSealError::ResponseBroken,
            CloudSealError::SealRefused,
        ] {
            assert!(!error.to_string().contains("送っていません"), "{error}");

            assert!(
                error.to_string().contains("（音声は保護して送りました）"),
                "{error}"
            );
        }
    }

    #[test]
    fn no_message_states_what_this_side_cannot_know() {
        for error in CloudSealError::ALL {
            let text = error.to_string();
            for claim in [
                "開かれていません",
                "入れ替わり",
                "切り替わりました",
                "使えます",
                "保護されていませんでした",
            ] {
                assert!(!text.contains(claim), "{error:?} says {claim:?}: {text}");
            }
        }
    }

    #[test]
    fn a_failure_to_seal_does_not_claim_a_reply_was_unprotected() {
        let text = CloudSealError::SealFailed.to_string();
        assert!(!text.contains("応答"), "{text}");
        assert!(text.contains("送信を中止"), "{text}");
    }

    #[test]
    fn stale_key_after_sealing_retries_exactly_once_then_stops_without_downgrade() {
        let mut state = RecordingSealState::default();
        decide_upload(&required(), &mut state, Some(enclave()), NoKeyReason::Other).unwrap();
        assert_eq!(
            on_gateway_error(&mut state, 409, "stale_enclave_key"),
            SealErrorAction::RefetchKeyAndRetry
        );
        assert_eq!(
            on_gateway_error(&mut state, 409, "stale_enclave_key"),
            SealErrorAction::StopWithGatewayError
        );

        assert_eq!(
            decide_upload(&optional(), &mut state, None, NoKeyReason::Other).unwrap_err(),
            CloudSealError::KeyLostAfterSend
        );
    }

    #[test]
    fn stale_key_without_having_sealed_is_not_a_sealing_matter() {
        let mut state = RecordingSealState::default();
        assert_eq!(
            on_gateway_error(&mut state, 409, "stale_enclave_key"),
            SealErrorAction::NotSealing
        );
    }

    #[test]
    fn a_broken_seal_stops_the_recording() {
        for code in ["sealed_response_missing", "sealed_response_unreadable"] {
            let mut state = RecordingSealState::default();
            assert_eq!(
                on_gateway_error(&mut state, 200, code),
                SealErrorAction::Stop(CloudSealError::ResponseBroken),
                "{code}"
            );
        }
        let mut state = RecordingSealState::default();
        assert_eq!(
            on_gateway_error(&mut state, 0, "seal_failed"),
            SealErrorAction::Stop(CloudSealError::SealFailed)
        );
    }

    #[test]
    fn being_told_to_seal_after_sealing_is_not_blamed_on_the_app() {
        let mut state = RecordingSealState::default();
        decide_upload(&required(), &mut state, Some(enclave()), NoKeyReason::Other).unwrap();
        assert_eq!(
            on_gateway_error(&mut state, 400, "sealed_required"),
            SealErrorAction::Stop(CloudSealError::SealRefused)
        );
        assert!(!CloudSealError::SealRefused.to_string().contains("更新"));
    }

    #[test]
    fn an_unsealed_send_that_is_refused_does_not_claim_the_audio_was_protected() {
        let mut state = RecordingSealState::default();
        assert_eq!(
            on_gateway_error(&mut state, 400, "sealed_required"),
            SealErrorAction::Stop(CloudSealError::PlaintextRefused)
        );
        assert!(!CloudSealError::PlaintextRefused
            .to_string()
            .contains("保護して送りました"));
    }

    #[test]
    fn everything_else_is_left_to_the_generic_rules() {
        let mut state = RecordingSealState::default();
        for (status, code) in [
            (401, "unauthorized"),
            (429, "cap_exceeded"),
            (502, "upstream_error"),
            (503, "server_busy"),
            (408, ""),
            (0, "client_misconfigured"),
        ] {
            assert_eq!(
                on_gateway_error(&mut state, status, code),
                SealErrorAction::NotSealing,
                "{status} {code}"
            );
        }
    }

    #[test]
    fn the_messages_match_the_android_client_word_for_word() {
        let expected = [
            (
                CloudSealError::Unavailable,
                "安全に送れる状態を確認できませんでした（音声は送っていません）",
            ),
            (
                CloudSealError::Outdated,
                "このアプリでは、いまのサーバーを確認できません。アプリを最新版に更新してください（音声は送っていません）",
            ),
            (
                CloudSealError::SealFailed,
                "音声を保護できなかったため、送信を中止しました",
            ),
            (
                CloudSealError::KeyLostAfterSend,
                "サーバーに送り直せませんでした。もう一度お試しください（音声は保護して送りました）",
            ),
            (
                CloudSealError::OutdatedAfterSend,
                "このアプリでは、いまのサーバーを確認できません。アプリを最新版に更新してください（音声は保護して送りました）",
            ),
            (
                CloudSealError::ResponseBroken,
                "保護された応答を受け取れませんでした。時間をおいて試してください（音声は保護して送りました）",
            ),
            (
                CloudSealError::SealRefused,
                "サーバーが、保護された送信として受け付けませんでした。時間をおいて試してください（音声は保護して送りました）",
            ),
            (
                CloudSealError::PlaintextRefused,
                "サーバーが、保護されていない送信を受け付けませんでした",
            ),
        ];
        assert_eq!(expected.len(), CloudSealError::ALL.len());
        for (error, text) in expected {
            assert_eq!(error.to_string(), text, "{error:?}");
        }
    }

    #[test]
    fn the_window_shows_the_same_sentence_in_japanese() {
        let translations: serde_json::Value = serde_json::from_str(include_str!(
            "../../../src/i18n/locales/ja/translation.json"
        ))
        .expect("ja/translation.json is valid JSON");
        let english: serde_json::Value = serde_json::from_str(include_str!(
            "../../../src/i18n/locales/en/translation.json"
        ))
        .expect("en/translation.json is valid JSON");
        for error in CloudSealError::ALL {
            let id = error.notice_id();
            assert_eq!(
                translations["errors"]["notice"][id].as_str(),
                Some(error.to_string().as_str()),
                "errors.notice.{id} in ja"
            );
            assert!(
                english["errors"]["notice"][id]
                    .as_str()
                    .is_some_and(|text| !text.is_empty()),
                "errors.notice.{id} is missing in en"
            );
        }
    }
}
