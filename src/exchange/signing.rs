use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

/// Sign a query string with HMAC-SHA256 (Binance style).
/// Returns hex-encoded signature.
pub fn sign_binance(query: &str, secret: &str) -> Result<String, String> {
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
        .map_err(|e| format!("HMAC error: {}", e))?;
    mac.update(query.as_bytes());
    Ok(hex::encode(mac.finalize().into_bytes()))
}

/// Sign a message with HMAC-SHA512 for Kraken.
/// Kraken uses: HMAC-SHA512(uri_path + SHA256(nonce + post_data), base64_decode(secret))
pub fn sign_kraken(uri_path: &str, nonce: u64, post_data: &str, secret_b64: &str) -> Result<String, String> {
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    use sha2::{Sha256, Sha512, Digest};
    use hmac::{Hmac, Mac};

    // Decode the base64 secret
    let secret_bytes = STANDARD.decode(secret_b64)
        .map_err(|e| format!("base64 decode error: {}", e))?;

    // SHA256(nonce + post_data)
    let mut sha256 = Sha256::new();
    sha256.update(nonce.to_string().as_bytes());
    sha256.update(post_data.as_bytes());
    let sha256_hash = sha256.finalize();

    // uri_path + sha256_hash
    let mut message = uri_path.as_bytes().to_vec();
    message.extend_from_slice(&sha256_hash);

    // HMAC-SHA512
    type HmacSha512 = Hmac<Sha512>;
    let mut mac = HmacSha512::new_from_slice(&secret_bytes)
        .map_err(|e| format!("HMAC error: {}", e))?;
    mac.update(&message);

    Ok(STANDARD.encode(mac.finalize().into_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_binance_sign() {
        let query = "symbol=BTCUSDT&side=BUY&type=LIMIT&timeInForce=GTC&quantity=0.001&price=50000&timestamp=1234567890000";
        let secret = "test_secret";
        let sig = sign_binance(query, secret).unwrap();
        assert!(!sig.is_empty());
        assert_eq!(sig.len(), 64);
    }
}
