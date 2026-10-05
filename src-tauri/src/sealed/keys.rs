use super::attestation::{self, AttestationResult, VerifiedEnclave};
use super::wire;
use log::{info, warn};
use serde_json::Value;
use std::sync::Mutex;

pub enum SourceError {
    Unsupported,
    Failed(String),
}

const UNSUPPORTED_COOLDOWN_SECONDS: i64 = 300;

const REUSE_SECONDS: i64 = 3000;

const REFRESH_MARGIN_SECONDS: i64 = 600;

pub trait AttestationSource {
    fn attestation(&self, nonce: &str) -> Result<Value, SourceError>;

    fn jwks(&self) -> Result<Vec<Value>, SourceError>;
}

struct CachedEnclave {
    enclave: VerifiedEnclave,
    reuse_until_seconds: i64,
}

#[derive(Default)]
struct State {
    cached: Option<CachedEnclave>,
    jwks: Option<Vec<Value>>,

    unsupported_until_seconds: i64,

    last_refusal_was_unknown_image: bool,
}

pub struct EnclaveKeyStore {
    audience: String,

    accepted_image_digests: Vec<String>,
    state: Mutex<State>,
}

impl EnclaveKeyStore {
    pub fn new(audience: impl Into<String>, accepted_image_digests: Vec<String>) -> Self {
        Self {
            audience: audience.into(),
            accepted_image_digests,
            state: Mutex::new(State::default()),
        }
    }

    pub fn prefetch(&self, source: &dyn AttestationSource, now_seconds: i64) {
        if let Some(left) = self.cached_remaining_seconds(now_seconds) {
            if left > REFRESH_MARGIN_SECONDS {
                return;
            }
        }
        self.fetch_and_cache(source, now_seconds);
    }

    pub fn acquire(
        &self,
        source: &dyn AttestationSource,
        now_seconds: i64,
    ) -> Option<VerifiedEnclave> {
        if let Some(enclave) = self.cached_valid(now_seconds) {
            return Some(enclave);
        }
        self.fetch_and_cache(source, now_seconds)
    }

    pub fn reacquire(
        &self,
        source: &dyn AttestationSource,
        now_seconds: i64,
    ) -> Option<VerifiedEnclave> {
        if let Ok(mut state) = self.state.lock() {
            state.cached = None;
        }
        self.fetch_and_cache(source, now_seconds)
    }

    pub fn has_valid_cache(&self, now_seconds: i64) -> bool {
        self.cached_valid(now_seconds).is_some()
    }

    fn cached_remaining_seconds(&self, now_seconds: i64) -> Option<i64> {
        self.state.lock().ok().and_then(|state| {
            state.cached.as_ref().and_then(|cached| {
                let until = cached
                    .reuse_until_seconds
                    .min(cached.enclave.expires_at_seconds);
                (until > now_seconds).then_some(until - now_seconds)
            })
        })
    }

    fn cached_valid(&self, now_seconds: i64) -> Option<VerifiedEnclave> {
        self.state.lock().ok().and_then(|state| {
            state.cached.as_ref().and_then(|cached| {
                let alive = now_seconds < cached.reuse_until_seconds
                    && now_seconds < cached.enclave.expires_at_seconds;
                alive.then(|| cached.enclave.clone())
            })
        })
    }

    fn fetch_and_cache(
        &self,
        source: &dyn AttestationSource,
        now_seconds: i64,
    ) -> Option<VerifiedEnclave> {
        let enclave = self.fetch_and_verify(source, now_seconds)?;
        if let Ok(mut state) = self.state.lock() {
            state.cached = Some(CachedEnclave {
                enclave: enclave.clone(),
                reuse_until_seconds: now_seconds + REUSE_SECONDS,
            });
        }
        Some(enclave)
    }

    pub fn last_refusal_was_unknown_image(&self) -> bool {
        self.state
            .lock()
            .map(|s| s.last_refusal_was_unknown_image)
            .unwrap_or(false)
    }

    fn fetch_and_verify(
        &self,
        source: &dyn AttestationSource,
        now_seconds: i64,
    ) -> Option<VerifiedEnclave> {
        if let Ok(mut state) = self.state.lock() {
            state.last_refusal_was_unknown_image = false;
        }
        if self
            .state
            .lock()
            .map(|s| now_seconds < s.unsupported_until_seconds)
            .unwrap_or(true)
        {
            return None;
        }
        let nonce = match wire::new_client_nonce() {
            Ok(n) => n,
            Err(e) => {
                warn!("[sealed] cannot generate a nonce: {}", e);
                return None;
            }
        };
        let body = match source.attestation(&nonce) {
            Ok(body) => body,
            Err(SourceError::Unsupported) => {
                if let Ok(mut state) = self.state.lock() {
                    state.unsupported_until_seconds = now_seconds + UNSUPPORTED_COOLDOWN_SECONDS;
                }
                info!(
                    "[sealed] gateway does not support sealing; not asking again for {}s",
                    UNSUPPORTED_COOLDOWN_SECONDS
                );
                return None;
            }
            Err(SourceError::Failed(e)) => {
                warn!("[sealed] could not fetch the declaration: {}", e);
                return None;
            }
        };

        let mut keys = self.cached_jwks(source)?;
        let mut result = self.verify(&body, &nonce, &keys, now_seconds);
        if matches!(&result, AttestationResult::Rejected { .. })
            && result.failed_labels() == "signing key"
        {
            keys = self.refresh_jwks(source)?;
            result = self.verify(&body, &nonce, &keys, now_seconds);
        }

        match result {
            AttestationResult::Verified { enclave, .. } => {
                info!(
                    "[sealed] verified key_id={} image={}",
                    enclave.key_id_hex, enclave.image_digest
                );
                Some(*enclave)
            }
            rejected => {
                let unknown_image = rejected.rejected_only_for_unknown_image();
                if let Ok(mut state) = self.state.lock() {
                    state.last_refusal_was_unknown_image = unknown_image;
                }
                if unknown_image {
                    info!("[sealed] the running image is not on this build's accepted list — this app is behind");
                } else {
                    warn!(
                        "[sealed] declaration rejected ({}) — not sealing to this key",
                        rejected.failed_labels()
                    );
                }
                None
            }
        }
    }

    fn verify(
        &self,
        body: &Value,
        nonce: &str,
        keys: &[Value],
        now_seconds: i64,
    ) -> AttestationResult {
        attestation::verify(
            body,
            nonce,
            keys,
            now_seconds,
            &self.accepted_image_digests,
            &self.audience,
        )
    }

    fn cached_jwks(&self, source: &dyn AttestationSource) -> Option<Vec<Value>> {
        if let Some(keys) = self.state.lock().ok().and_then(|s| s.jwks.clone()) {
            return Some(keys);
        }
        self.refresh_jwks(source)
    }

    fn refresh_jwks(&self, source: &dyn AttestationSource) -> Option<Vec<Value>> {
        match source.jwks() {
            Ok(keys) => {
                if let Ok(mut state) = self.state.lock() {
                    state.jwks = Some(keys.clone());
                }
                Some(keys)
            }
            Err(SourceError::Unsupported) | Err(SourceError::Failed(_)) => {
                warn!("[sealed] could not fetch Google's key set");
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::attestation::DEFAULT_AUDIENCE;
    use super::super::test_support::{enclave_key, jwks, Declaration, IMAGE_DIGEST, NOW};
    use super::*;
    use serde_json::json;
    use std::sync::Mutex as StdMutex;

    struct FakeSource {
        declaration: Declaration,

        unsupported: bool,
        attestation_fails: bool,
        jwks_fails: bool,

        stale_jwks_once: bool,
        nonces: StdMutex<Vec<String>>,
        jwks_calls: StdMutex<usize>,
    }

    impl FakeSource {
        fn new() -> Self {
            Self {
                declaration: Declaration::default(),
                unsupported: false,
                attestation_fails: false,
                jwks_fails: false,
                stale_jwks_once: false,
                nonces: StdMutex::new(Vec::new()),
                jwks_calls: StdMutex::new(0),
            }
        }

        fn nonces(&self) -> Vec<String> {
            self.nonces.lock().unwrap().clone()
        }

        fn attestation_calls(&self) -> usize {
            self.nonces.lock().unwrap().len()
        }

        fn jwks_calls(&self) -> usize {
            *self.jwks_calls.lock().unwrap()
        }
    }

    impl AttestationSource for FakeSource {
        fn attestation(&self, nonce: &str) -> Result<Value, SourceError> {
            self.nonces.lock().unwrap().push(nonce.to_string());
            if self.unsupported {
                return Err(SourceError::Unsupported);
            }
            if self.attestation_fails {
                return Err(SourceError::Failed("boom".to_string()));
            }
            Ok(self.declaration.body_for(nonce))
        }

        fn jwks(&self) -> Result<Vec<Value>, SourceError> {
            let mut calls = self.jwks_calls.lock().unwrap();
            *calls += 1;
            if self.jwks_fails {
                return Err(SourceError::Failed("boom".to_string()));
            }
            if self.stale_jwks_once && *calls == 1 {
                return Ok(vec![
                    json!({"kty": "RSA", "kid": "an-older-key", "n": "AA", "e": "AQAB"}),
                ]);
            }
            Ok(jwks())
        }
    }

    fn store() -> EnclaveKeyStore {
        store_accepting(vec![IMAGE_DIGEST.to_string()])
    }

    fn store_accepting(accepted: Vec<String>) -> EnclaveKeyStore {
        EnclaveKeyStore::new(DEFAULT_AUDIENCE, accepted)
    }

    #[test]
    fn a_key_is_verified_before_it_is_handed_out() {
        let source = FakeSource::new();
        let enclave = store().acquire(&source, NOW).expect("no key");
        assert_eq!(enclave_key(), enclave.public_key);
    }

    #[test]
    fn a_prefetched_key_costs_no_round_trip_at_send_time() {
        let source = FakeSource::new();
        let store = store();
        store.prefetch(&source, NOW);
        assert_eq!(1, source.attestation_calls());

        assert!(store.acquire(&source, NOW).is_some());
        assert_eq!(1, source.attestation_calls());
    }

    #[test]
    fn a_verified_declaration_is_reused_within_the_window() {
        let source = FakeSource::new();
        let store = store();
        store.prefetch(&source, NOW);
        assert!(store.acquire(&source, NOW).is_some());
        assert!(store.acquire(&source, NOW + REUSE_SECONDS - 1).is_some());
        assert_eq!(1, source.attestation_calls());
    }

    #[test]
    fn a_declaration_with_room_left_is_not_renewed() {
        let source = FakeSource::new();
        let store = store();
        store.prefetch(&source, NOW);
        store.prefetch(&source, NOW + REUSE_SECONDS - REFRESH_MARGIN_SECONDS - 1);
        assert_eq!(1, source.attestation_calls());
    }

    #[test]
    fn a_declaration_near_its_end_is_renewed_before_it_is_needed() {
        let source = FakeSource::new();
        let store = store();
        store.prefetch(&source, NOW);
        let near_end = NOW + REUSE_SECONDS - REFRESH_MARGIN_SECONDS + 1;
        store.prefetch(&source, near_end);
        assert_eq!(2, source.attestation_calls(), "余裕を切ったら取り直す");

        assert!(store.acquire(&source, near_end).is_some());
        assert_eq!(2, source.attestation_calls(), "送信時に往復を足さない");
    }

    #[test]
    fn a_failed_renewal_keeps_the_one_we_already_verified() {
        let mut source = FakeSource::new();
        let store = store();
        store.prefetch(&source, NOW);
        let near_end = NOW + REUSE_SECONDS - REFRESH_MARGIN_SECONDS + 1;

        source.attestation_fails = true;
        store.prefetch(&source, near_end);

        assert!(
            store.acquire(&source, near_end).is_some(),
            "失敗した取り直しが、有効なキャッシュを捨てていない"
        );
        assert!(store.has_valid_cache(near_end));
    }

    #[test]
    fn the_cache_flag_says_whether_a_send_would_be_free() {
        let source = FakeSource::new();
        let store = store();
        assert!(!store.has_valid_cache(NOW), "取る前は当たらない");
        store.prefetch(&source, NOW);
        assert!(store.has_valid_cache(NOW), "取った直後は当たる");
        assert!(
            store.has_valid_cache(NOW + REUSE_SECONDS - 1),
            "窓の内側は当たる"
        );
        assert!(
            !store.has_valid_cache(NOW + REUSE_SECONDS + 1),
            "窓の外は外れる"
        );
    }

    #[test]
    fn a_declaration_past_the_window_is_refetched_under_a_fresh_nonce() {
        let source = FakeSource::new();
        let store = store();
        assert!(store.acquire(&source, NOW).is_some());
        assert!(store.acquire(&source, NOW + REUSE_SECONDS + 1).is_some());
        assert_eq!(2, source.attestation_calls());

        let nonces = source.nonces();
        assert_ne!(nonces[0], nonces[1]);
    }

    #[test]
    fn prefetching_twice_does_not_fetch_twice() {
        let source = FakeSource::new();
        let store = store();
        store.prefetch(&source, NOW);
        store.prefetch(&source, NOW);
        assert_eq!(1, source.attestation_calls());
    }

    #[test]
    fn an_expired_prefetch_is_replaced_before_sending() {
        let source = FakeSource::new();
        let store = store();
        store.prefetch(&source, NOW);

        assert!(store.acquire(&source, NOW + 7200).is_none());
        assert_eq!(2, source.attestation_calls());
    }

    #[test]
    fn a_stale_key_error_always_refetches() {
        let source = FakeSource::new();
        let store = store();
        store.prefetch(&source, NOW);
        assert!(store.reacquire(&source, NOW).is_some());
        assert_eq!(2, source.attestation_calls());

        assert!(store.acquire(&source, NOW).is_some());
        assert_eq!(2, source.attestation_calls());
    }

    #[test]
    fn a_declaration_that_fails_verification_yields_no_key() {
        let mut source = FakeSource::new();
        source.declaration.declared_key = vec![0xAA; 32];
        assert!(store().acquire(&source, NOW).is_none());
    }

    #[test]
    fn an_image_we_do_not_accept_yields_no_key() {
        let other = "sha256:0000000000000000000000000000000000000000000000000000000000000000";
        let source = FakeSource::new();
        assert!(store_accepting(vec![other.to_string()])
            .acquire(&source, NOW)
            .is_none());
    }

    #[test]
    fn an_image_we_do_not_accept_is_reported_as_this_app_being_behind() {
        let other = "sha256:0000000000000000000000000000000000000000000000000000000000000000";
        let source = FakeSource::new();
        let store = store_accepting(vec![other.to_string()]);
        assert!(store.acquire(&source, NOW).is_none());
        assert!(store.last_refusal_was_unknown_image());
    }

    #[test]
    fn a_declaration_that_fails_verification_is_not_reported_as_an_old_app() {
        let mut source = FakeSource::new();
        source.declaration.declared_key = vec![0xAA; 32];
        let store = store();
        assert!(store.acquire(&source, NOW).is_none());
        assert!(!store.last_refusal_was_unknown_image());
    }

    #[test]
    fn the_old_app_verdict_does_not_outlive_the_attempt_that_earned_it() {
        let other = "sha256:0000000000000000000000000000000000000000000000000000000000000000";
        let mut source = FakeSource::new();
        let store = store_accepting(vec![other.to_string()]);
        assert!(store.acquire(&source, NOW).is_none());
        assert!(store.last_refusal_was_unknown_image());

        source.attestation_fails = true;
        assert!(store.acquire(&source, NOW).is_none());
        assert!(!store.last_refusal_was_unknown_image());
    }

    #[test]
    fn an_empty_accepted_list_yields_no_key() {
        let source = FakeSource::new();
        assert!(store_accepting(Vec::new()).acquire(&source, NOW).is_none());
    }

    #[test]
    fn a_swap_window_can_carry_both_the_old_and_the_new_image() {
        let older = "sha256:0000000000000000000000000000000000000000000000000000000000000000";
        let source = FakeSource::new();
        let store = store_accepting(vec![older.to_string(), IMAGE_DIGEST.to_string()]);
        assert!(store.acquire(&source, NOW).is_some());
    }

    #[test]
    fn a_failed_fetch_yields_no_key_and_is_retried_later() {
        let mut source = FakeSource::new();
        source.attestation_fails = true;
        let store = store();
        assert!(store.acquire(&source, NOW).is_none());
        assert!(store.acquire(&source, NOW).is_none());

        assert_eq!(2, source.attestation_calls());
    }

    #[test]
    fn an_unreachable_key_set_yields_no_key() {
        let mut source = FakeSource::new();
        source.jwks_fails = true;
        assert!(store().acquire(&source, NOW).is_none());
    }

    #[test]
    fn the_key_set_is_fetched_once_and_reused() {
        let source = FakeSource::new();
        let store = store();
        assert!(store.acquire(&source, NOW).is_some());
        assert!(store.acquire(&source, NOW).is_some());
        assert_eq!(1, source.jwks_calls());
    }

    #[test]
    fn a_rotated_signing_key_is_picked_up_without_failing() {
        let mut source = FakeSource::new();
        source.stale_jwks_once = true;
        let store = store();
        assert!(store.acquire(&source, NOW).is_some());
        assert_eq!(2, source.jwks_calls());
    }

    #[test]
    fn a_gateway_without_sealing_is_not_asked_again_straight_away() {
        let mut source = FakeSource::new();
        source.unsupported = true;
        let store = store();
        assert!(store.acquire(&source, NOW).is_none());
        store.prefetch(&source, NOW);
        assert!(store.acquire(&source, NOW).is_none());
        assert!(store.reacquire(&source, NOW).is_none());
        assert_eq!(1, source.attestation_calls());
    }

    #[test]
    fn the_gateway_is_asked_again_once_the_cooldown_lapses() {
        let mut source = FakeSource::new();
        source.unsupported = true;
        let store = store();
        assert!(store.acquire(&source, NOW).is_none());
        assert!(store
            .acquire(&source, NOW + UNSUPPORTED_COOLDOWN_SECONDS + 1)
            .is_none());
        assert_eq!(2, source.attestation_calls());
    }
}
