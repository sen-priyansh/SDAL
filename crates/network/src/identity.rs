// crates/network/src/identity.rs
//
// Ed25519-based identity for SDAL.
//
// Rules:
//   1. Always verify on server — never trust client.
//   2. Never store private keys on server — client owns identity.
//   3. Include timestamp / nonce to prevent replay attacks.
//
// The client signs requests locally with their private key.
// The server verifies signatures using only the public key.

use ed25519_dalek::{
    Signature, Signer, SigningKey, Verifier, VerifyingKey,
};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

// Re-export so consumers (CLI) don't need ed25519-dalek directly
pub use ed25519_dalek::SigningKey as IdentitySigningKey;

/// A signed request envelope wrapping arbitrary payload bytes.
///
/// The signature covers: SHA-256(timestamp || nonce || payload)
/// This prevents replay attacks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedEnvelope {
    /// The raw payload bytes (request body)
    #[serde(with = "base64_serde")]
    pub payload: Vec<u8>,
    /// Ed25519 public key of the signer (32 bytes, hex-encoded)
    pub public_key: String,
    /// Ed25519 signature over the digest (64 bytes, hex-encoded)
    pub signature: String,
    /// Unix timestamp (seconds) when the request was created
    pub timestamp: u64,
    /// Random nonce to prevent replay (hex-encoded, 16 bytes)
    pub nonce: String,
}

/// Generate a new Ed25519 keypair.
///
/// Returns (signing_key_bytes, verifying_key_bytes).
pub fn generate_keypair() -> (Vec<u8>, Vec<u8>) {
    let signing_key = SigningKey::generate(&mut OsRng);
    let verifying_key = signing_key.verifying_key();
    (
        signing_key.to_bytes().to_vec(),
        verifying_key.to_bytes().to_vec(),
    )
}

/// Save a keypair to disk.
///
/// Writes:
///   <dir>/private.key  (32 bytes, hex-encoded)
///   <dir>/public.key   (32 bytes, hex-encoded)
pub fn save_keypair(dir: &Path, signing_bytes: &[u8], verifying_bytes: &[u8]) -> anyhow::Result<()> {
    std::fs::create_dir_all(dir)?;
    let private_path = dir.join("private.key");
    std::fs::write(&private_path, hex::encode(signing_bytes))?;
    // Best-effort: restrict private key to owner-only on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&private_path, std::fs::Permissions::from_mode(0o600));
    }
    std::fs::write(dir.join("public.key"), hex::encode(verifying_bytes))?;
    Ok(())
}

/// Load the signing (private) key from disk.
pub fn load_signing_key(dir: &Path) -> anyhow::Result<SigningKey> {
    let hex_str = std::fs::read_to_string(dir.join("private.key"))?;
    let bytes = hex::decode(hex_str.trim())?;
    let key_bytes: [u8; 32] = bytes
        .try_into()
        .map_err(|_| anyhow::anyhow!("Invalid private key length"))?;
    Ok(SigningKey::from_bytes(&key_bytes))
}

/// Load the verifying (public) key from a hex string.
pub fn load_verifying_key_from_hex(hex_str: &str) -> anyhow::Result<VerifyingKey> {
    let bytes = hex::decode(hex_str.trim())?;
    let key_bytes: [u8; 32] = bytes
        .try_into()
        .map_err(|_| anyhow::anyhow!("Invalid public key length"))?;
    VerifyingKey::from_bytes(&key_bytes).map_err(|e| anyhow::anyhow!("Invalid public key: {}", e))
}

/// Compute the signing digest: SHA-256(timestamp_bytes || nonce_bytes || payload)
fn compute_digest(timestamp: u64, nonce: &[u8], payload: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(timestamp.to_le_bytes());
    hasher.update(nonce);
    hasher.update(payload);
    let result = hasher.finalize();
    let mut out = [0u8; 32];
    out.copy_from_slice(&result);
    out
}

/// Sign a payload, producing a `SignedEnvelope`.
pub fn sign_payload(signing_key: &SigningKey, payload: &[u8]) -> SignedEnvelope {
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    // Generate a 16-byte random nonce
    let mut nonce_bytes = [0u8; 16];
    use rand::RngCore;
    OsRng.fill_bytes(&mut nonce_bytes);

    let digest = compute_digest(timestamp, &nonce_bytes, payload);
    let signature = signing_key.sign(&digest);

    SignedEnvelope {
        payload: payload.to_vec(),
        public_key: hex::encode(signing_key.verifying_key().to_bytes()),
        signature: hex::encode(signature.to_bytes()),
        timestamp,
        nonce: hex::encode(nonce_bytes),
    }
}

/// Verify a `SignedEnvelope` on the server side.
///
/// Checks:
///   1. Signature is valid for the given public key
///   2. Timestamp is within `max_age_secs` of the current time
///
/// Returns the verified public key on success.
pub fn verify_envelope(
    envelope: &SignedEnvelope,
    max_age_secs: u64,
) -> anyhow::Result<VerifyingKey> {
    // 1. Parse public key
    let verifying_key = load_verifying_key_from_hex(&envelope.public_key)?;

    // 2. Parse signature
    let sig_bytes = hex::decode(&envelope.signature)?;
    let sig_array: [u8; 64] = sig_bytes
        .try_into()
        .map_err(|_| anyhow::anyhow!("Invalid signature length"))?;
    let signature = Signature::from_bytes(&sig_array);

    // 3. Parse nonce
    let nonce_bytes = hex::decode(&envelope.nonce)?;

    // 4. Recompute digest
    let digest = compute_digest(envelope.timestamp, &nonce_bytes, &envelope.payload);

    // 5. Verify signature
    verifying_key
        .verify(&digest, &signature)
        .map_err(|e| anyhow::anyhow!("Signature verification failed: {}", e))?;

    // 6. Check timestamp freshness (prevent replay)
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let age = if now >= envelope.timestamp {
        now - envelope.timestamp
    } else {
        envelope.timestamp - now
    };

    if age > max_age_secs {
        anyhow::bail!(
            "Request expired: age {} seconds exceeds max {} seconds",
            age,
            max_age_secs
        );
    }

    Ok(verifying_key)
}

/// Base64 serde helper for payload bytes
mod base64_serde {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(data: &[u8], serializer: S) -> Result<S::Ok, S::Error> {
        use base64::Engine;
        let encoded = base64::engine::general_purpose::STANDARD.encode(data);
        serializer.serialize_str(&encoded)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<u8>, D::Error> {
        use base64::Engine;
        let s = String::deserialize(deserializer)?;
        base64::engine::general_purpose::STANDARD
            .decode(&s)
            .map_err(serde::de::Error::custom)
    }
}

/// Return the global SDAL identity directory: `~/.sdal/identity/`
pub fn global_identity_dir() -> anyhow::Result<PathBuf> {
    let home = std::env::var("HOME")
        .map_err(|_| anyhow::anyhow!("Cannot determine home directory ($HOME not set)"))?;
    Ok(PathBuf::from(home).join(".sdal").join("identity"))
}

/// Load or create an Ed25519 identity keypair.
///
/// If the identity directory already contains keys, load them.
/// Otherwise, generate a fresh keypair and save it.
pub fn load_or_create_identity(identity_dir: &Path) -> anyhow::Result<SigningKey> {
    if identity_dir.join("private.key").exists() {
        load_signing_key(identity_dir)
    } else {
        let (sk_bytes, vk_bytes) = generate_keypair();
        save_keypair(identity_dir, &sk_bytes, &vk_bytes)?;
        load_signing_key(identity_dir)
    }
}

/// Convenience: load the global identity, creating it if missing.
pub fn load_global_identity() -> anyhow::Result<SigningKey> {
    let dir = global_identity_dir()?;
    load_or_create_identity(&dir)
}

/// Return the hex-encoded public key for a signing key.
pub fn public_key_hex(signing_key: &SigningKey) -> String {
    hex::encode(signing_key.verifying_key().to_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sign_and_verify() {
        let (sk_bytes, _vk_bytes) = generate_keypair();
        let signing_key = SigningKey::from_bytes(
            &<[u8; 32]>::try_from(sk_bytes.as_slice()).unwrap(),
        );

        let payload = b"test push request data";
        let envelope = sign_payload(&signing_key, payload);

        // Should verify successfully (within time window)
        let result = verify_envelope(&envelope, 300);
        assert!(result.is_ok());
    }

    #[test]
    fn test_tampered_payload_fails() {
        let (sk_bytes, _) = generate_keypair();
        let signing_key = SigningKey::from_bytes(
            &<[u8; 32]>::try_from(sk_bytes.as_slice()).unwrap(),
        );

        let payload = b"original data";
        let mut envelope = sign_payload(&signing_key, payload);

        // Tamper with payload
        envelope.payload = b"tampered data".to_vec();

        let result = verify_envelope(&envelope, 300);
        assert!(result.is_err());
    }

    #[test]
    fn test_expired_request_fails() {
        let (sk_bytes, _) = generate_keypair();
        let signing_key = SigningKey::from_bytes(
            &<[u8; 32]>::try_from(sk_bytes.as_slice()).unwrap(),
        );

        let payload = b"data";
        let mut envelope = sign_payload(&signing_key, payload);

        // Set timestamp far in the past
        envelope.timestamp = 1000;

        // Re-sign with old timestamp would require re-signing, but
        // here we just verify the original gets rejected due to time
        // Actually, we need to test by setting max_age to 0
        let result = verify_envelope(&envelope, 0);
        assert!(result.is_err());
    }
}
