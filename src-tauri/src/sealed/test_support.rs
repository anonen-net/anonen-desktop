use super::attestation::{verify, AttestationResult, DEFAULT_AUDIENCE, ISSUER};
use super::wire;
use rsa::pkcs8::DecodePrivateKey;
use rsa::signature::{SignatureEncoding, Signer};
use serde_json::{json, Value};

const SIGNING_KEY_PEM: &str = "-----BEGIN PRIVATE KEY-----\n\
MIIEvAIBADANBgkqhkiG9w0BAQEFAASCBKYwggSiAgEAAoIBAQDE4i9aPVhcLMWi\n\
64298fNLCO6o2g0hfDiRkZ5lpWlPrKieOjcnwWyVqCxatHJgN54ykb18C/RW5Azt\n\
FqUTqA7UtLYAHTUy67W3bJWTkluf1zK9XhgzO6wI30RyCWfMB8wjY8fVZSqy668K\n\
22MTdymqBDD0W1UvbvjuSt8H4pQQiYNo6800EhM8RWYfomO0fIxKgnOCe3n/h4/c\n\
5KrEZeXfIJ6q7oJZYI/REbrd8JNz/r7InAN2raTOS1ff3fJ+x5Nup35TVe/16Q/q\n\
aKlpMIXxQqHA49TCqFIOdguKYj9nDFaD6xH4KjiABxnRfhZR8L0W/R49gOZP7u1U\n\
Id++ekmjAgMBAAECggEAU1rZpnBzcO7pq9zjkUgW860v8eADkCo5/vNyZuF9sSRN\n\
7VODV3sOQFxHjw13oSHbAdjvKs4a4BDEIjqkoCpWQRrTNUAobksaD7LSrvxXe1hO\n\
XJsyTrqUTttL7KXrwpeIh3EuwTuINOEQpl2U2EBqrNcCbOogKilnDeg3/ewhONBo\n\
k4l6pSJpbJakZ1y45NH+a2V4MPYXrVjunJ1bZszsekp4haKPbpwcZAArBXAYA6xl\n\
qxzAVcL8rFfXgVpz21tLXNK2WY526Kb8opGw7Ku3dl+n85QK929Y8YeqY/97D1CZ\n\
Q7kimNmP48ugJ5uRB1Wbsce4+1H3IxlzfkqEEfSnoQKBgQD236xoyJdd74tIvK+O\n\
pUsY+ngo4LKUQ1897ZzvgnulUD89KLKbT6Ui+LD8N0wgJYHKue9xuRjVwZ+UPYYR\n\
svWwYP2Ak5MrdKMKErfKIJrLAfdxgyuNJ3jzhBOo6IReVaVL7ry0ZKUBRaouhPJs\n\
EQw2/37uIkgTK8rSVxiNX/ykQwKBgQDMKWv9rtYHzCbtILrHkxg7hY3lW/FnkutN\n\
PO80QWjB2tGT67WVh9CgnjphlocF9lJQpzzgB+yvpigKWkWP0WL4SW1hp4/L9qOG\n\
Zwwq2VPBdPulDeK8Ce+K0bLc+jEvBzqcZfKSKrPX/Dt5U/A/+JRftzPVqzuIedzT\n\
e08GFxcfIQKBgBi71CQzZKbupv/El7os6Vc5UOyXhozAKzyjH8QHSKgJYCyadTqG\n\
Qj88t06to5zc5SIQeuLj7o8L9Rb/Cv2e9mqTbRKCto32A5ewxGUl2xxXuBjRUX20\n\
5LNRGj8j/I8O+Z6f7pwMMVPtTN2YSEl3c8fW1Tq6iIoTKyDjxPv1D/TNAoGAKRu+\n\
DtqBZhiJTf3lHgJUQsflhmU4+m3uyyKV16PEACSbRnmTmoQ6Ud0KTFQO4/hxIqRJ\n\
8aIm2kdOkKcUkpzKnMvMJnmizqLe0lU1cftIjg+MC/P7z3kArqcCqNQHyzxdZNhy\n\
O1Q12iuDni50lazr00oEOeW8mwJOoJaXltZXZYECgYAQJSeWBWqdYeUiOty6s91e\n\
7Xbc+Ma/hkk480h2HZZZIolqOvXzM+C3t5IrJjL3Rd3VWa8tGoBmThUKV5FR9duC\n\
j5MbMpUykfNQocUxItFeJBOoMWEWQrBfo7hnI7hYF+7ztJdrEvzB2qjNvgHOZRQR\n\
6F9UPY6OiV/pWvnYVhsopg==\n\
-----END PRIVATE KEY-----\n";

pub const KID: &str = "test-kid-2026";
pub const NOW: i64 = 1_755_300_000;
pub const NONCE: &str = "wtxr71MT7UcNfSLBNJwayvncNwQArqQD";
pub const IMAGE_DIGEST: &str =
    "sha256:111e9edc8feed1d8c64ee2daaf5952f07cf3f1bb6b883d0575ebd01f446c6933";

fn private_key() -> rsa::RsaPrivateKey {
    rsa::RsaPrivateKey::from_pkcs8_pem(SIGNING_KEY_PEM).unwrap()
}

pub fn jwks() -> Vec<Value> {
    use rsa::traits::PublicKeyParts;
    let public = rsa::RsaPublicKey::from(private_key());
    vec![json!({
        "kty": "RSA",
        "kid": KID,
        "n": wire::b64url_no_pad(&public.n().to_bytes_be()),
        "e": wire::b64url_no_pad(&public.e().to_bytes_be()),
    })]
}

pub fn enclave_key() -> Vec<u8> {
    (0..32u8)
        .map(|i| i.wrapping_mul(7).wrapping_add(3))
        .collect()
}

pub struct Declaration {
    pub alg: &'static str,
    pub kid: &'static str,
    pub issuer: &'static str,
    pub audience: &'static str,
    pub issued_at: i64,
    pub expires_at: i64,

    pub bound_key: Vec<u8>,

    pub declared_key: Vec<u8>,
    pub declared_key_id: Option<String>,
    pub eat_nonce: Option<Value>,
    pub suite: String,
    pub dbgstat: &'static str,
    pub swname: &'static str,
    pub restart_policy: &'static str,

    pub image_digest: Option<&'static str>,

    pub support_attributes: Option<Vec<&'static str>>,
    pub break_signature: bool,
    pub token: Option<&'static str>,
}

impl Default for Declaration {
    fn default() -> Self {
        Self {
            alg: "RS256",
            kid: KID,
            issuer: ISSUER,
            audience: DEFAULT_AUDIENCE,
            issued_at: NOW - 10,
            expires_at: NOW + 3600,
            bound_key: enclave_key(),
            declared_key: enclave_key(),
            declared_key_id: None,
            eat_nonce: None,
            suite: wire::SUITE.to_string(),
            dbgstat: "disabled-since-boot",
            swname: "CONFIDENTIAL_SPACE",
            restart_policy: "Never",
            image_digest: Some(IMAGE_DIGEST),
            support_attributes: Some(vec!["LATEST", "STABLE", "USABLE"]),
            break_signature: false,
            token: None,
        }
    }
}

impl Declaration {
    pub fn body_for(&self, client_nonce: &str) -> Value {
        let token = self.token.map(|t| t.to_string()).unwrap_or_else(|| {
            let header = json!({"alg": self.alg, "kid": self.kid, "typ": "JWT"});
            let mut submods = json!({
                "container": {
                    "restart_policy": self.restart_policy,
                }
            });
            if let Some(digest) = self.image_digest {
                submods["container"]["image_digest"] = json!(digest);
            }
            if let Some(attributes) = &self.support_attributes {
                submods["confidential_space"] = json!({"support_attributes": attributes});
            }
            let payload = json!({
                "iss": self.issuer,
                "aud": self.audience,
                "iat": self.issued_at,
                "exp": self.expires_at,
                "eat_nonce": self.eat_nonce.clone().unwrap_or_else(|| {
                    Value::String(wire::bound_nonce(client_nonce, &self.bound_key))
                }),
                "dbgstat": self.dbgstat,
                "swname": self.swname,
                "submods": submods,
            });
            let signed = format!(
                "{}.{}",
                wire::b64url_no_pad(header.to_string().as_bytes()),
                wire::b64url_no_pad(payload.to_string().as_bytes())
            );
            let mut signature = rsa::pkcs1v15::SigningKey::<sha2::Sha256>::new(private_key())
                .sign(signed.as_bytes())
                .to_vec();
            if self.break_signature {
                signature[0] ^= 0xff;
            }
            format!("{}.{}", signed, wire::b64url_no_pad(&signature))
        });

        json!({
            "token": token,
            "public_key": wire::b64url_no_pad(&self.declared_key),
            "key_id": self
                .declared_key_id
                .clone()
                .unwrap_or_else(|| wire::key_id_hex(&self.declared_key)),
            "suite": self.suite,
        })
    }

    pub fn body(&self) -> Value {
        self.body_for(NONCE)
    }

    pub fn check(&self) -> AttestationResult {
        self.check_accepting(&[IMAGE_DIGEST.to_string()])
    }

    pub fn check_accepting(&self, accepted: &[String]) -> AttestationResult {
        verify(
            &self.body(),
            NONCE,
            &jwks(),
            NOW,
            accepted,
            DEFAULT_AUDIENCE,
        )
    }

    pub fn rejected_for(&self, label: &str) {
        match self.check() {
            AttestationResult::Verified { .. } => panic!(
                "accepted a declaration that should have been refused ({})",
                label
            ),
            result => assert_eq!(label, result.failed_labels()),
        }
    }
}
