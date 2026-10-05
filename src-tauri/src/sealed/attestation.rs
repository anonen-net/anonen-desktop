use super::wire;
use rsa::signature::Verifier;
use serde_json::Value;

pub const ISSUER: &str = "https://confidentialcomputing.googleapis.com";
pub const DISCOVERY_URL: &str =
    "https://confidentialcomputing.googleapis.com/.well-known/openid-configuration";
pub const DEFAULT_AUDIENCE: &str = "https://anonen.net/attestation/v1";

const CLOCK_SKEW_SECONDS: i64 = 60;

const REQUIRED_SUPPORT_ATTRIBUTE: &str = "STABLE";

pub const IMAGE_DIGEST_LABEL: &str = "image digest";

pub fn parse_accepted_digests(raw: &str) -> Vec<String> {
    raw.split([',', ' ', '\n'])
        .map(|d| d.trim().to_ascii_lowercase())
        .filter(|d| !d.is_empty())
        .collect()
}

#[derive(Debug, Clone)]
pub struct Check {
    pub ok: bool,
    pub label: &'static str,
}

#[derive(Debug, Clone)]
pub struct VerifiedEnclave {
    pub public_key: Vec<u8>,
    pub key_id_hex: String,

    pub image_digest: String,

    pub expires_at_seconds: i64,
}

pub enum AttestationResult {
    Verified {
        enclave: Box<VerifiedEnclave>,
        checks: Vec<Check>,
    },
    Rejected {
        checks: Vec<Check>,
    },
}

impl AttestationResult {
    pub fn failed_labels(&self) -> String {
        match self {
            AttestationResult::Verified { .. } => String::new(),
            AttestationResult::Rejected { checks } => checks
                .iter()
                .filter(|c| !c.ok)
                .map(|c| c.label)
                .collect::<Vec<_>>()
                .join("/"),
        }
    }

    pub fn rejected_only_for_unknown_image(&self) -> bool {
        match self {
            AttestationResult::Verified { .. } => false,
            AttestationResult::Rejected { checks } => {
                let mut failed = checks.iter().filter(|c| !c.ok).map(|c| c.label);
                failed.next() == Some(IMAGE_DIGEST_LABEL) && failed.next().is_none()
            }
        }
    }
}

pub fn verify(
    body: &Value,
    client_nonce: &str,
    jwks: &[Value],
    now_seconds: i64,
    accepted_image_digests: &[String],
    audience: &str,
) -> AttestationResult {
    let mut checks: Vec<Check> = Vec::new();

    let token = body.get("token").and_then(Value::as_str).unwrap_or("");
    if token.trim().is_empty() {
        return reject(checks, "token present");
    }
    let jwt = match Jwt::parse(token) {
        Some(j) => j,
        None => return reject(checks, "token is a JWT"),
    };

    let algorithm = jwt.header.get("alg").and_then(Value::as_str).unwrap_or("");
    if algorithm != "RS256" {
        return reject(checks, "signature algorithm");
    }
    let kid = jwt.header.get("kid").and_then(Value::as_str).unwrap_or("");
    let jwk = match jwks
        .iter()
        .find(|k| k.get("kid").and_then(Value::as_str) == Some(kid))
    {
        Some(k) => k,
        None => return reject(checks, "signing key"),
    };
    let signature_ok = rs256_verify(jwk, &jwt.signed, &jwt.signature);
    checks.push(Check {
        ok: signature_ok,
        label: "Google signature",
    });
    if !signature_ok {
        return AttestationResult::Rejected { checks };
    }

    let issuer = jwt.payload.get("iss").and_then(Value::as_str).unwrap_or("");
    checks.push(Check {
        ok: issuer == ISSUER,
        label: "issuer",
    });

    let issued_at = jwt.payload.get("iat").and_then(Value::as_i64).unwrap_or(0);
    let expires_at = jwt.payload.get("exp").and_then(Value::as_i64).unwrap_or(0);
    checks.push(Check {
        ok: now_seconds >= issued_at - CLOCK_SKEW_SECONDS
            && now_seconds <= expires_at + CLOCK_SKEW_SECONDS,
        label: "validity window",
    });

    let public_key = body
        .get("public_key")
        .and_then(Value::as_str)
        .and_then(wire::decode_public_key);
    let declared_key_id = body.get("key_id").and_then(Value::as_str).unwrap_or("");
    checks.push(Check {
        ok: public_key
            .as_ref()
            .is_some_and(|pk| declared_key_id == wire::key_id_hex(pk)),
        label: "declared public key",
    });

    let suite = body.get("suite").and_then(Value::as_str).unwrap_or("");

    checks.push(Check {
        ok: suite == wire::SUITE,
        label: "sealing suite",
    });

    checks.push(Check {
        ok: public_key.as_ref().is_some_and(|pk| {
            nonce_matches(
                jwt.payload.get("eat_nonce"),
                &wire::bound_nonce(client_nonce, pk),
            )
        }),
        label: "nonce/key binding",
    });

    let aud = jwt.payload.get("aud").and_then(Value::as_str).unwrap_or("");
    checks.push(Check {
        ok: aud == audience,
        label: "audience",
    });

    let submods = jwt.payload.get("submods");
    let container = submods.and_then(|s| s.get("container"));
    checks.push(Check {
        ok: jwt.payload.get("dbgstat").and_then(Value::as_str) == Some("disabled-since-boot"),
        label: "debug disabled",
    });
    checks.push(Check {
        ok: jwt.payload.get("swname").and_then(Value::as_str) == Some("CONFIDENTIAL_SPACE"),
        label: "runtime environment",
    });

    checks.push(Check {
        ok: submods
            .and_then(|s| s.get("confidential_space"))
            .and_then(|s| s.get("support_attributes"))
            .and_then(Value::as_array)
            .is_some_and(|attributes| {
                attributes
                    .iter()
                    .filter_map(Value::as_str)
                    .any(|a| a == REQUIRED_SUPPORT_ATTRIBUTE)
            }),
        label: "image channel",
    });
    checks.push(Check {
        ok: container
            .and_then(|c| c.get("restart_policy"))
            .and_then(Value::as_str)
            == Some("Never"),
        label: "restart policy",
    });

    let image_digest = container
        .and_then(|c| c.get("image_digest"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    checks.push(Check {
        ok: !image_digest.is_empty()
            && accepted_image_digests.contains(&image_digest.to_ascii_lowercase()),
        label: IMAGE_DIGEST_LABEL,
    });

    let public_key = match public_key {
        Some(pk) if checks.iter().all(|c| c.ok) => pk,
        _ => return AttestationResult::Rejected { checks },
    };
    AttestationResult::Verified {
        enclave: Box::new(VerifiedEnclave {
            key_id_hex: wire::key_id_hex(&public_key),
            public_key,
            image_digest,
            expires_at_seconds: expires_at,
        }),
        checks,
    }
}

fn nonce_matches(claim: Option<&Value>, expected: &str) -> bool {
    if expected.is_empty() {
        return false;
    }
    match claim {
        Some(Value::String(s)) => s == expected,
        Some(Value::Array(items)) => items
            .iter()
            .any(|v| v.as_str().is_some_and(|s| s == expected)),
        _ => false,
    }
}

fn rs256_verify(jwk: &Value, signed: &[u8], signature: &[u8]) -> bool {
    let field = |name: &str| {
        jwk.get(name)
            .and_then(Value::as_str)
            .and_then(|v| wire::b64url_decode(v).ok())
    };
    let (Some(n), Some(e)) = (field("n"), field("e")) else {
        return false;
    };
    let key = match rsa::RsaPublicKey::new(
        rsa::BigUint::from_bytes_be(&n),
        rsa::BigUint::from_bytes_be(&e),
    ) {
        Ok(k) => k,
        Err(_) => return false,
    };
    let Ok(signature) = rsa::pkcs1v15::Signature::try_from(signature) else {
        return false;
    };
    rsa::pkcs1v15::VerifyingKey::<sha2::Sha256>::new(key)
        .verify(signed, &signature)
        .is_ok()
}

fn reject(mut checks: Vec<Check>, label: &'static str) -> AttestationResult {
    checks.push(Check { ok: false, label });
    AttestationResult::Rejected { checks }
}

struct Jwt {
    header: Value,
    payload: Value,

    signed: Vec<u8>,
    signature: Vec<u8>,
}

impl Jwt {
    fn parse(token: &str) -> Option<Jwt> {
        let parts: Vec<&str> = token.split('.').collect();
        if parts.len() != 3 {
            return None;
        }
        let decode = |part: &str| -> Option<Value> {
            serde_json::from_slice(&wire::b64url_decode(part).ok()?).ok()
        };
        let header = decode(parts[0])?;
        let payload = decode(parts[1])?;
        if !header.is_object() || !payload.is_object() {
            return None;
        }
        Some(Jwt {
            header,
            payload,
            signed: format!("{}.{}", parts[0], parts[1]).into_bytes(),
            signature: wire::b64url_decode(parts[2]).ok()?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_support::{enclave_key, Declaration, IMAGE_DIGEST, NONCE, NOW};
    use super::*;
    use serde_json::json;

    #[test]
    fn a_good_declaration_is_accepted() {
        match Declaration::default().check() {
            AttestationResult::Verified { enclave, checks } => {
                assert_eq!(enclave_key(), enclave.public_key);
                assert_eq!(wire::key_id_hex(&enclave_key()), enclave.key_id_hex);
                assert_eq!(IMAGE_DIGEST, enclave.image_digest);
                assert_eq!(NOW + 3600, enclave.expires_at_seconds);
                assert!(checks.iter().all(|c| c.ok));
            }
            result => panic!("refused a good declaration: {}", result.failed_labels()),
        }
    }

    #[test]
    fn a_substituted_public_key_is_refused() {
        Declaration {
            declared_key: vec![0xAA; 32],
            ..Default::default()
        }
        .rejected_for("nonce/key binding");
    }

    #[test]
    fn a_declaration_answering_another_nonce_is_refused() {
        Declaration {
            eat_nonce: Some(Value::String(wire::bound_nonce(
                "someone-elses",
                &enclave_key(),
            ))),
            ..Default::default()
        }
        .rejected_for("nonce/key binding");
    }

    #[test]
    fn eat_nonce_may_be_an_array() {
        let bound = wire::bound_nonce(NONCE, &enclave_key());
        let accepted = Declaration {
            eat_nonce: Some(json!(["something-else", bound])),
            ..Default::default()
        };
        assert!(matches!(
            accepted.check(),
            AttestationResult::Verified { .. }
        ));

        Declaration {
            eat_nonce: Some(json!(["something-else", "another"])),
            ..Default::default()
        }
        .rejected_for("nonce/key binding");
    }

    #[test]
    fn a_missing_eat_nonce_is_refused() {
        Declaration {
            eat_nonce: Some(Value::Null),
            ..Default::default()
        }
        .rejected_for("nonce/key binding");
    }

    #[test]
    fn a_missing_token_is_refused() {
        Declaration {
            token: Some(""),
            ..Default::default()
        }
        .rejected_for("token present");
    }

    #[test]
    fn a_token_that_is_not_a_jwt_is_refused() {
        Declaration {
            token: Some("not.a.jwt"),
            ..Default::default()
        }
        .rejected_for("token is a JWT");
    }

    #[test]
    fn an_unsigned_token_is_refused() {
        Declaration {
            alg: "none",
            ..Default::default()
        }
        .rejected_for("signature algorithm");
    }

    #[test]
    fn an_unknown_signing_key_is_refused() {
        Declaration {
            kid: "not-a-google-kid",
            ..Default::default()
        }
        .rejected_for("signing key");
    }

    #[test]
    fn a_broken_signature_is_refused() {
        Declaration {
            break_signature: true,
            ..Default::default()
        }
        .rejected_for("Google signature");
    }

    #[test]
    fn nothing_after_the_signature_is_evaluated_when_it_fails() {
        let result = Declaration {
            break_signature: true,
            issuer: "https://example.invalid",
            swname: "SOMETHING_ELSE",
            ..Default::default()
        }
        .check();
        assert_eq!("Google signature", result.failed_labels());
    }

    #[test]
    fn another_issuer_is_refused() {
        Declaration {
            issuer: "https://confidentialcomputing.googleapis.com.example.invalid",
            ..Default::default()
        }
        .rejected_for("issuer");
    }

    #[test]
    fn an_expired_declaration_is_refused() {
        Declaration {
            issued_at: NOW - 7200,
            expires_at: NOW - 3600,
            ..Default::default()
        }
        .rejected_for("validity window");
    }

    #[test]
    fn a_declaration_from_the_future_is_refused() {
        Declaration {
            issued_at: NOW + 3600,
            expires_at: NOW + 7200,
            ..Default::default()
        }
        .rejected_for("validity window");
    }

    #[test]
    fn a_clock_off_by_less_than_a_minute_still_works() {
        assert!(matches!(
            Declaration {
                issued_at: NOW + 30,
                expires_at: NOW - 30,
                ..Default::default()
            }
            .check(),
            AttestationResult::Verified { .. }
        ));
    }

    #[test]
    fn a_key_id_that_does_not_match_the_key_is_refused() {
        Declaration {
            declared_key_id: Some("00000000000000000000000000000000".to_string()),
            ..Default::default()
        }
        .rejected_for("declared public key");
    }

    #[test]
    fn a_public_key_of_the_wrong_length_is_refused() {
        let result = Declaration {
            declared_key: vec![0x11; 31],
            ..Default::default()
        }
        .check();

        assert_eq!(
            "declared public key/nonce/key binding",
            result.failed_labels()
        );
    }

    #[test]
    fn an_unknown_suite_is_refused() {
        Declaration {
            suite: "HPKE-X25519-HKDF-SHA256-AES128GCM".to_string(),
            ..Default::default()
        }
        .rejected_for("sealing suite");
    }

    #[test]
    fn a_token_minted_for_someone_else_is_refused() {
        Declaration {
            audience: "https://example.invalid/attestation/v1",
            ..Default::default()
        }
        .rejected_for("audience");
    }

    #[test]
    fn a_debuggable_machine_is_refused() {
        Declaration {
            dbgstat: "enabled",
            ..Default::default()
        }
        .rejected_for("debug disabled");
    }

    #[test]
    fn a_non_confidential_runtime_is_refused() {
        Declaration {
            swname: "GCE",
            ..Default::default()
        }
        .rejected_for("runtime environment");
    }

    #[test]
    fn a_restartable_container_is_refused() {
        Declaration {
            restart_policy: "Always",
            ..Default::default()
        }
        .rejected_for("restart policy");
    }

    #[test]
    fn an_experimental_image_is_refused() {
        Declaration {
            support_attributes: Some(vec!["LATEST", "USABLE"]),
            ..Default::default()
        }
        .rejected_for("image channel");
    }

    #[test]
    fn a_missing_image_channel_is_refused() {
        Declaration {
            support_attributes: None,
            ..Default::default()
        }
        .rejected_for("image channel");
    }

    #[test]
    fn an_image_outside_the_accepted_list_is_refused() {
        let other = "sha256:0000000000000000000000000000000000000000000000000000000000000000";
        match Declaration::default().check_accepting(&[other.to_string()]) {
            AttestationResult::Verified { .. } => panic!("sealed to an image we never accepted"),
            result => assert_eq!("image digest", result.failed_labels()),
        }
    }

    #[test]
    fn an_empty_accepted_list_refuses_everything() {
        match Declaration::default().check_accepting(&[]) {
            AttestationResult::Verified { .. } => panic!("an empty list accepted an image"),
            result => assert_eq!("image digest", result.failed_labels()),
        }
    }

    #[test]
    fn a_missing_image_digest_is_refused() {
        Declaration {
            image_digest: None,
            ..Default::default()
        }
        .rejected_for("image digest");
    }

    #[test]
    fn a_swap_window_accepts_both_the_old_and_the_new_image() {
        let older = "sha256:0000000000000000000000000000000000000000000000000000000000000000";
        let accepted = vec![older.to_string(), IMAGE_DIGEST.to_string()];
        assert!(matches!(
            Declaration::default().check_accepting(&accepted),
            AttestationResult::Verified { .. }
        ));
    }

    #[test]
    fn the_accepted_list_tolerates_spacing_and_case() {
        assert_eq!(
            vec![IMAGE_DIGEST.to_string()],
            parse_accepted_digests(&format!("  {}  ", IMAGE_DIGEST.to_uppercase()))
        );
        assert_eq!(2, parse_accepted_digests("sha256:aa, sha256:bb").len());
        assert!(parse_accepted_digests("").is_empty());
        assert!(parse_accepted_digests("  ,  ").is_empty());
    }

    #[test]
    fn several_failures_are_all_reported() {
        let result = Declaration {
            swname: "GCE",
            restart_policy: "Always",
            ..Default::default()
        }
        .check();
        assert_eq!("runtime environment/restart policy", result.failed_labels());
    }

    #[test]
    fn a_failure_report_carries_no_values() {
        let result = Declaration {
            declared_key: vec![0xAA; 32],
            audience: "https://example.invalid/attestation/v1",
            ..Default::default()
        }
        .check();
        let labels = result.failed_labels();
        assert!(!labels.contains("example.invalid"));
        assert!(!labels.contains(&wire::b64url_no_pad(&[0xAA; 32])));
    }
}
