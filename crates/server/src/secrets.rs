//! At-rest encryption for credentials (SSH private keys, kubeconfigs, env
//! var values): XChaCha20-Poly1305 with a key derived from `PJX_MASTER_KEY`.
//!
//! The derive-then-encrypt layer is deliberately small — it is the seam where
//! per-secret data keys + KMS wrapping slot in later without a data
//! migration (ciphertexts carry a version prefix).

use base64::{engine::general_purpose::STANDARD as B64, Engine};
use chacha20poly1305::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    XChaCha20Poly1305, XNonce,
};
use sha2::{Digest, Sha256};

const VERSION: u8 = 1;
const NONCE_LEN: usize = 24;

#[derive(Clone)]
pub struct MasterKey([u8; 32]);

impl MasterKey {
    /// Derive from the `PJX_MASTER_KEY` env var (any sufficiently long
    /// string; we hash it to 32 bytes).
    pub fn from_env() -> anyhow::Result<Self> {
        let raw = std::env::var("PJX_MASTER_KEY").map_err(|_| {
            anyhow::anyhow!(
                "PJX_MASTER_KEY is required (any long random string; it encrypts \
                 stored credentials — losing it means reconnecting every server)"
            )
        })?;
        if raw.len() < 16 {
            anyhow::bail!("PJX_MASTER_KEY must be at least 16 characters");
        }
        Ok(Self(Sha256::digest(raw.as_bytes()).into()))
    }

    fn cipher(&self) -> XChaCha20Poly1305 {
        XChaCha20Poly1305::new((&self.0).into())
    }

    /// Encrypt to a base64 string: `version || nonce || ciphertext`.
    pub fn encrypt(&self, plaintext: &[u8]) -> anyhow::Result<String> {
        let nonce = XChaCha20Poly1305::generate_nonce(&mut OsRng);
        let ct = self
            .cipher()
            .encrypt(&nonce, plaintext)
            .map_err(|_| anyhow::anyhow!("encryption failed"))?;
        let mut out = Vec::with_capacity(1 + NONCE_LEN + ct.len());
        out.push(VERSION);
        out.extend_from_slice(&nonce);
        out.extend_from_slice(&ct);
        Ok(B64.encode(out))
    }

    pub fn decrypt(&self, encoded: &str) -> anyhow::Result<Vec<u8>> {
        let raw = B64.decode(encoded)?;
        if raw.len() < 1 + NONCE_LEN || raw[0] != VERSION {
            anyhow::bail!("unrecognized ciphertext format");
        }
        let nonce = XNonce::from_slice(&raw[1..1 + NONCE_LEN]);
        self.cipher()
            .decrypt(nonce, &raw[1 + NONCE_LEN..])
            .map_err(|_| anyhow::anyhow!("decryption failed (wrong PJX_MASTER_KEY?)"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key() -> MasterKey {
        MasterKey(Sha256::digest(b"test master key").into())
    }

    #[test]
    fn round_trip() {
        let k = key();
        let ct = k.encrypt(b"super secret ssh key").unwrap();
        assert_eq!(k.decrypt(&ct).unwrap(), b"super secret ssh key");
    }

    #[test]
    fn distinct_nonces() {
        let k = key();
        assert_ne!(k.encrypt(b"x").unwrap(), k.encrypt(b"x").unwrap());
    }

    #[test]
    fn wrong_key_fails() {
        let ct = key().encrypt(b"data").unwrap();
        let other = MasterKey(Sha256::digest(b"different key").into());
        assert!(other.decrypt(&ct).is_err());
    }

    #[test]
    fn tampered_ciphertext_fails() {
        let k = key();
        let ct = k.encrypt(b"data").unwrap();
        let mut raw = B64.decode(&ct).unwrap();
        let last = raw.len() - 1;
        raw[last] ^= 1;
        assert!(k.decrypt(&B64.encode(raw)).is_err());
    }
}
