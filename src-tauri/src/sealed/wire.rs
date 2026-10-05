use base64::Engine;
use hpke::{
    aead::ChaCha20Poly1305 as HpkeChaCha, kdf::HkdfSha256, kem::X25519HkdfSha256, Deserializable,
    Kem as KemTrait, OpModeS, Serializable,
};
use sha2::{Digest, Sha256};

pub const CONTENT_TYPE: &str = "application/vnd.anonen.sealed";

pub const SUITE: &str = "HPKE-X25519-HKDF-SHA256-CHACHA20POLY1305";

pub const PUBLIC_KEY_BYTES: usize = 32;

const MAGIC: &[u8] = b"ANON1";
const TRANSCRIBE_INFO: &[u8] = b"anonen/transcribe/v1";
const RESPONSE_AAD: &[u8] = b"anonen/response/v1";
const KEY_ID_BYTES: usize = 16;
const RESPONSE_KEY_BYTES: usize = 32;
const RESPONSE_NONCE_BYTES: usize = 12;
const MAX_HEADER_BYTES: usize = 8192;

type Kem = X25519HkdfSha256;

pub struct SealedEnvelope {
    pub body: Vec<u8>,
    pub response_key: [u8; RESPONSE_KEY_BYTES],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SealedError(pub &'static str);

impl std::fmt::Display for SealedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for SealedError {}

pub fn seal(
    public_key: &[u8],
    request_id: &str,
    model: &str,
    language: Option<&str>,
    audio: &[u8],
) -> Result<SealedEnvelope, SealedError> {
    if public_key.len() != PUBLIC_KEY_BYTES {
        return Err(SealedError("enclave public key is not 32 bytes"));
    }
    let mut response_key = [0u8; RESPONSE_KEY_BYTES];
    getrandom::fill(&mut response_key).map_err(|_| SealedError("no system randomness"))?;

    let mut header = serde_json::Map::new();
    header.insert("request_id".into(), request_id.into());
    header.insert("response_key".into(), b64url_no_pad(&response_key).into());

    if model.trim().is_empty() {
        return Err(SealedError("model is required (api-v1 R23)"));
    }
    header.insert("model".into(), model.into());

    if let Some(l) = language.filter(|l| !l.trim().is_empty() && *l != "auto") {
        header.insert("language".into(), l.into());
    }
    let header = serde_json::Value::Object(header).to_string().into_bytes();
    if header.len() > MAX_HEADER_BYTES {
        return Err(SealedError("sealed header too large"));
    }

    let mut plaintext = Vec::with_capacity(4 + header.len() + audio.len());
    plaintext.extend_from_slice(&(header.len() as u32).to_be_bytes());
    plaintext.extend_from_slice(&header);
    plaintext.extend_from_slice(audio);

    let recipient = <Kem as KemTrait>::PublicKey::from_bytes(public_key)
        .map_err(|_| SealedError("enclave public key is not a valid X25519 point"))?;

    let (encapped, ciphertext) = hpke::single_shot_seal::<HpkeChaCha, HkdfSha256, Kem>(
        &OpModeS::Base,
        &recipient,
        TRANSCRIBE_INFO,
        &plaintext,
        &[],
    )
    .map_err(|_| SealedError("cannot seal request"))?;
    let encapped = encapped.to_bytes();

    let mut body =
        Vec::with_capacity(MAGIC.len() + KEY_ID_BYTES + encapped.len() + ciphertext.len());
    body.extend_from_slice(MAGIC);
    body.extend_from_slice(&key_id(public_key));
    body.extend_from_slice(&encapped);
    body.extend_from_slice(&ciphertext);

    Ok(SealedEnvelope { body, response_key })
}

pub fn open_response(response_key: &[u8], body: &[u8]) -> Result<Vec<u8>, SealedError> {
    use chacha20poly1305::{
        aead::{Aead, KeyInit, Payload},
        ChaCha20Poly1305, Nonce,
    };

    if body.len() <= RESPONSE_NONCE_BYTES {
        return Err(SealedError("malformed sealed response"));
    }
    let (nonce, sealed) = body.split_at(RESPONSE_NONCE_BYTES);
    let cipher = ChaCha20Poly1305::new_from_slice(response_key)
        .map_err(|_| SealedError("response key is not 32 bytes"))?;
    let nonce = Nonce::try_from(nonce).map_err(|_| SealedError("malformed sealed response"))?;
    cipher
        .decrypt(
            &nonce,
            Payload {
                msg: sealed,
                aad: RESPONSE_AAD,
            },
        )
        .map_err(|_| SealedError("cannot open sealed response"))
}

pub fn key_id(public_key: &[u8]) -> [u8; KEY_ID_BYTES] {
    let digest = Sha256::digest(public_key);
    let mut out = [0u8; KEY_ID_BYTES];
    out.copy_from_slice(&digest[..KEY_ID_BYTES]);
    out
}

pub fn key_id_hex(public_key: &[u8]) -> String {
    key_id(public_key)
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect()
}

pub fn bound_nonce(client_nonce: &str, public_key: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(client_nonce.as_bytes());
    digest.update(public_key);
    b64url_no_pad(&digest.finalize())
}

pub fn new_client_nonce() -> Result<String, SealedError> {
    let mut raw = [0u8; 24];
    getrandom::fill(&mut raw).map_err(|_| SealedError("no system randomness"))?;
    Ok(b64url_no_pad(&raw))
}

pub fn decode_public_key(value: &str) -> Option<Vec<u8>> {
    let raw = b64url_decode(value).ok()?;
    (raw.len() == PUBLIC_KEY_BYTES).then_some(raw)
}

pub fn b64url_no_pad(raw: &[u8]) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(raw)
}

pub fn b64url_decode(value: &str) -> Result<Vec<u8>, base64::DecodeError> {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use hpke::OpModeR;
    use serde_json::Value;

    fn vectors() -> Value {
        serde_json::from_str(include_str!("../../tests/fixtures/sealed_vectors.json")).unwrap()
    }

    fn text(path: &[&str]) -> String {
        let mut node = vectors();
        for key in path {
            node = node.get(key).unwrap().clone();
        }
        node.as_str().unwrap().to_string()
    }

    fn bytes(path: &[&str]) -> Vec<u8> {
        let hex = text(path);
        (0..hex.len() / 2)
            .map(|i| u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).unwrap())
            .collect()
    }

    fn public_key() -> Vec<u8> {
        bytes(&["enclave_public_key_hex"])
    }

    fn open_sealed_request(body: &[u8]) -> (Value, Vec<u8>) {
        let private_key = bytes(&["enclave_private_key_hex"]);
        let recipient = <Kem as KemTrait>::PrivateKey::from_bytes(&private_key).unwrap();
        let prefix = 5 + 16;
        let enc = <Kem as KemTrait>::EncappedKey::from_bytes(&body[prefix..prefix + 32]).unwrap();
        let plaintext = hpke::single_shot_open::<HpkeChaCha, HkdfSha256, Kem>(
            &OpModeR::Base,
            &recipient,
            &enc,
            b"anonen/transcribe/v1",
            &body[prefix + 32..],
            &[],
        )
        .unwrap();

        let header_len = u32::from_be_bytes(plaintext[..4].try_into().unwrap()) as usize;
        let header: Value = serde_json::from_slice(&plaintext[4..4 + header_len]).unwrap();
        (header, plaintext[4 + header_len..].to_vec())
    }

    #[test]
    fn opens_a_request_python_sealed() {
        let (header, audio) = open_sealed_request(&bytes(&["sealed_request", "hex"]));

        assert_eq!(
            text(&["sealed_request", "header", "request_id"]),
            header["request_id"].as_str().unwrap()
        );
        assert_eq!(
            text(&["sealed_request", "header", "response_key"]),
            header["response_key"].as_str().unwrap()
        );
        assert_eq!("ja", header["language"].as_str().unwrap());
        assert_eq!(bytes(&["sealed_request", "audio_hex"]), audio);
    }

    #[test]
    fn opens_a_response_python_sealed() {
        let payload = open_response(
            &bytes(&["sealed_response", "response_key_hex"]),
            &bytes(&["sealed_response", "hex"]),
        )
        .unwrap();
        assert_eq!(
            text(&["sealed_response", "payload_utf8"]),
            String::from_utf8(payload).unwrap()
        );
    }

    #[test]
    fn suite_matches_the_server() {
        assert_eq!(text(&["suite"]), SUITE);
        assert_eq!(text(&["content_type"]), CONTENT_TYPE);
    }

    #[test]
    fn key_id_matches_the_server() {
        assert_eq!(text(&["key_id_hex"]), key_id_hex(&public_key()));
    }

    #[test]
    fn bound_nonce_matches_the_server() {
        assert_eq!(
            text(&["bound_nonce", "expected"]),
            bound_nonce(&text(&["bound_nonce", "client_nonce"]), &public_key())
        );
    }

    #[test]
    fn our_seal_opens_the_server_way() {
        let mut audio = b"RIFF".to_vec();
        audio.extend((0..200u32).map(|i| i as u8));
        let envelope = seal(
            &public_key(),
            "0d5b9f2e-0000-4111-8222-333344445555",
            "openai/whisper-1",
            Some("ja"),
            &audio,
        )
        .unwrap();

        assert_eq!(b"ANON1", &envelope.body[..5]);
        assert_eq!(key_id(&public_key()), envelope.body[5..21]);

        let (header, opened) = open_sealed_request(&envelope.body);
        assert_eq!(
            "0d5b9f2e-0000-4111-8222-333344445555",
            header["request_id"].as_str().unwrap()
        );
        assert_eq!("openai/whisper-1", header["model"].as_str().unwrap());
        assert_eq!(audio, opened);

        assert_eq!(
            b64url_no_pad(&envelope.response_key),
            header["response_key"].as_str().unwrap()
        );
    }

    #[test]
    fn auto_language_is_not_declared() {
        let envelope = seal(
            &public_key(),
            "0d5b9f2e-0000-4111-8222-333344445555",
            "openai/whisper-1",
            Some("auto"),
            &[0u8; 8],
        )
        .unwrap();
        let (header, _) = open_sealed_request(&envelope.body);
        assert!(header.get("language").is_none());

        assert_eq!("openai/whisper-1", header["model"].as_str().unwrap());
    }

    #[test]
    fn a_missing_model_is_not_sealed() {
        for id in ["", "   "] {
            assert!(
                seal(
                    &public_key(),
                    "0d5b9f2e-0000-4111-8222-333344445555",
                    id,
                    None,
                    &[0u8; 8],
                )
                .is_err(),
                "an empty model must not reach the wire: {:?}",
                id
            );
        }
    }

    #[test]
    fn audio_does_not_appear_in_the_body() {
        let envelope = seal(
            &public_key(),
            "0d5b9f2e-0000-4111-8222-333344445555",
            "openai/whisper-1",
            None,
            b"SECRETAUDIOMARKER",
        )
        .unwrap();
        let as_text = String::from_utf8_lossy(&envelope.body);
        assert!(
            !as_text.contains("SECRETAUDIOMARKER"),
            "plaintext survived into the sealed body"
        );
    }

    #[test]
    fn a_tampered_response_does_not_open() {
        let mut body = bytes(&["sealed_response", "hex"]);
        let last = body.len() - 1;
        body[last] = body[last].wrapping_add(1);
        assert!(open_response(&bytes(&["sealed_response", "response_key_hex"]), &body).is_err());
    }

    #[test]
    fn another_key_does_not_open_the_response() {
        assert!(open_response(&[9u8; 32], &bytes(&["sealed_response", "hex"])).is_err());
    }

    #[test]
    fn a_too_short_response_does_not_open() {
        assert!(open_response(&[0u8; 32], &[0u8; 8]).is_err());
    }

    #[test]
    fn a_wrong_length_key_is_not_sealed_to() {
        assert!(seal(
            &[0u8; 16],
            "0d5b9f2e-0000-4111-8222-333344445555",
            "openai/whisper-1",
            None,
            &[0u8; 4]
        )
        .is_err());
    }

    #[test]
    fn a_misshapen_public_key_is_not_taken() {
        assert!(decode_public_key("").is_none());
        assert!(decode_public_key("!!!not-base64!!!").is_none());
        assert!(decode_public_key(&b64url_no_pad(&[0u8; 31])).is_none());
        assert_eq!(
            Some(public_key()),
            decode_public_key(&text(&["enclave_public_key_b64url"]))
        );
    }

    #[test]
    fn the_nonce_fits_the_server_pattern_and_changes() {
        let nonces: Vec<String> = (0..50).map(|_| new_client_nonce().unwrap()).collect();
        for nonce in &nonces {
            assert!(
                (16..=64).contains(&nonce.len())
                    && nonce
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-'),
                "nonce has the wrong shape: {}",
                nonce
            );
        }
        let unique: std::collections::HashSet<&String> = nonces.iter().collect();
        assert_eq!(nonces.len(), unique.len());
    }
}
