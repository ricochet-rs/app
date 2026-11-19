//! Encrypts environment variables for ricochet, which accepts them only encrypted with its public key.

use crate::api::{ApiError, Result};
use base64::Engine;
use base64::prelude::BASE64_STANDARD;
use rsa::pkcs1::DecodeRsaPublicKey;
use rsa::traits::PublicKeyParts;
use rsa::{Oaep, RsaPublicKey};
use std::collections::HashMap;

/// `{base64(OAEP-SHA256(name)): base64(OAEP-SHA256(value))}`, the body `PATCH .../env-vars` takes.
pub fn encrypt_pair(pem: &str, name: &str, value: &str) -> Result<HashMap<String, String>> {
    let key = RsaPublicKey::from_pkcs1_pem(pem.trim()).map_err(|error| {
        ApiError::Failed(format!("The server sent an invalid public key: {error}"))
    })?;
    // OAEP with SHA-256 fits the key size less two digests and two bytes: 190 bytes for 2048 bits.
    let limit = key.size().saturating_sub(2 * 32 + 2);
    for (what, text) in [("name", name), ("value", value)] {
        if text.len() > limit {
            return Err(ApiError::Failed(format!(
                "The {what} is {} bytes, and ricochet accepts at most {limit}.",
                text.len()
            )));
        }
    }
    let encrypt = |text: &str| {
        key.encrypt(
            &mut rsa::rand_core::OsRng,
            Oaep::new::<rsa::sha2::Sha256>(),
            text.as_bytes(),
        )
        .map(|bytes| BASE64_STANDARD.encode(bytes))
        .map_err(|error| ApiError::Failed(error.to_string()))
    };
    Ok(HashMap::from([(encrypt(name)?, encrypt(value)?)]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rsa::pkcs1::EncodeRsaPublicKey;
    use rsa::{RsaPrivateKey, pkcs8::LineEnding};

    #[test]
    fn pairs_decrypt_with_the_private_key() {
        let private = RsaPrivateKey::new(&mut rsa::rand_core::OsRng, 2048).unwrap();
        let pem = private
            .to_public_key()
            .to_pkcs1_pem(LineEnding::LF)
            .unwrap();
        let pairs = encrypt_pair(&pem, "TOKEN", "secret").unwrap();
        let (name, value) = pairs.into_iter().next().unwrap();
        let decrypt = |text: &str| {
            let bytes = BASE64_STANDARD.decode(text).unwrap();
            private
                .decrypt(Oaep::new::<rsa::sha2::Sha256>(), &bytes)
                .unwrap()
        };
        assert_eq!(decrypt(&name), b"TOKEN");
        assert_eq!(decrypt(&value), b"secret");
        assert!(encrypt_pair(&pem, "LONG", &"x".repeat(191)).is_err());
    }
}
