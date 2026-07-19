//! GitHub webhook signature verification (`X-Hub-Signature-256`).

use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

/// Verify a webhook payload against the shared secret. `signature_header` is
/// the raw `X-Hub-Signature-256` value (`sha256=<hex>`), compared in constant
/// time.
pub fn verify_signature(secret: &[u8], payload: &[u8], signature_header: &str) -> bool {
    let Some(hex_sig) = signature_header.strip_prefix("sha256=") else {
        return false;
    };
    let Ok(expected) = hex::decode(hex_sig) else {
        return false;
    };
    let mut mac = HmacSha256::new_from_slice(secret).expect("hmac accepts any key length");
    mac.update(payload);
    mac.verify_slice(&expected).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_valid_signature() {
        let secret = b"It's a Secret to Everybody";
        let payload = b"Hello, World!";
        // Known-answer test vector from GitHub's webhook docs.
        let header = "sha256=757107ea0eb2509fc211221cce984b8a37570b6d7586c22c46f4379c8b043e17";
        assert!(verify_signature(secret, payload, header));
    }

    #[test]
    fn rejects_tampered_payload() {
        let secret = b"It's a Secret to Everybody";
        let header = "sha256=757107ea0eb2509fc211221cce984b8a37570b6d7586c22c46f4379c8b043e17";
        assert!(!verify_signature(secret, b"Hello, World?", header));
    }

    #[test]
    fn rejects_malformed_header() {
        assert!(!verify_signature(b"s", b"p", "sha1=abcd"));
        assert!(!verify_signature(b"s", b"p", "sha256=nothex"));
    }
}
