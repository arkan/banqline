use anyhow::{Context, Result};
use rsa::RsaPrivateKey;
use rsa::pkcs1::DecodeRsaPrivateKey;
use rsa::pkcs8::DecodePrivateKey;

/// Loads an RSA private key from a PEM-encoded file.
///
/// Supports both PKCS#1 (`RSA PRIVATE KEY`) and PKCS#8 (`PRIVATE KEY`) formats.
/// The file must contain exactly one PEM block.
pub fn load_private_key(path: &str) -> Result<RsaPrivateKey> {
    let data =
        std::fs::read(path).with_context(|| format!("reading private key file: {}", path))?;

    let pem = pem::parse(data).context("decoding PEM block: no valid PEM data found")?;

    match pem.tag() {
        "RSA PRIVATE KEY" => {
            RsaPrivateKey::from_pkcs1_der(pem.contents()).context("parsing PKCS#1 private key")
        }
        "PRIVATE KEY" => {
            RsaPrivateKey::from_pkcs8_der(pem.contents()).context("parsing PKCS#8 private key")
        }
        other => Err(anyhow::anyhow!("unsupported PEM block type: {}", other)),
    }
}
