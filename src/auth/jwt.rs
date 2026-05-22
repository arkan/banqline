use anyhow::{Context, Result};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use rsa::RsaPrivateKey;
use rsa::pkcs1v15::SigningKey;
use rsa::signature::RandomizedSigner;
use rsa::signature::SignatureEncoding;
use serde::Serialize;
use sha2::Sha256;
use std::time::{SystemTime, UNIX_EPOCH};

/// JWT header for RS256 signing.
#[derive(Serialize)]
struct JwtHeader {
    alg: String,
    typ: String,
    kid: String,
}

/// JWT claims for Enable Banking API authentication.
#[derive(Serialize)]
struct JwtClaims {
    iss: String,
    aud: String,
    iat: i64,
    exp: i64,
}

/// Generates a RS256-signed JWT for authenticating with the Enable Banking API.
///
/// The header includes the application ID as the key ID (`kid`).
/// Claims are issued for `enablebanking.com`, targeted at `api.enablebanking.com`,
/// and expire 1 hour (3600 seconds) from the current time.
pub fn generate_jwt(key: &RsaPrivateKey, application_id: &str) -> Result<String> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock before unix epoch")?
        .as_secs() as i64;

    let header = JwtHeader {
        alg: "RS256".to_string(),
        typ: "JWT".to_string(),
        kid: application_id.to_string(),
    };

    let claims = JwtClaims {
        iss: "enablebanking.com".to_string(),
        aud: "api.enablebanking.com".to_string(),
        iat: now,
        exp: now + 3600,
    };

    let header_json = serde_json::to_vec(&header).context("marshaling JWT header")?;
    let claims_json = serde_json::to_vec(&claims).context("marshaling JWT claims")?;

    let header_b64 = URL_SAFE_NO_PAD.encode(&header_json);
    let claims_b64 = URL_SAFE_NO_PAD.encode(&claims_json);

    let signing_input = format!("{}.{}", header_b64, claims_b64);

    let signing_key = SigningKey::<Sha256>::new(key.clone());
    let signature = signing_key.sign_with_rng(&mut rand::thread_rng(), signing_input.as_bytes());

    let signature_b64 = URL_SAFE_NO_PAD.encode(signature.to_vec());

    Ok(format!("{}.{}", signing_input, signature_b64))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rsa::pkcs1v15::{Signature, VerifyingKey};
    use rsa::signature::Verifier;

    #[test]
    fn generate_jwt_signature_verifies_with_rs256_input() {
        let key = RsaPrivateKey::new(&mut rand::thread_rng(), 2048).unwrap();

        let token = generate_jwt(&key, "test-app").unwrap();
        let parts: Vec<&str> = token.split('.').collect();
        assert_eq!(parts.len(), 3);

        let signing_input = format!("{}.{}", parts[0], parts[1]);
        let signature_bytes = URL_SAFE_NO_PAD.decode(parts[2]).unwrap();
        let signature = Signature::try_from(signature_bytes.as_slice()).unwrap();
        let verifying_key = VerifyingKey::<Sha256>::new(key.to_public_key());

        verifying_key
            .verify(signing_input.as_bytes(), &signature)
            .unwrap();
    }
}
