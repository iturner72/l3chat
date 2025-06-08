#[cfg(feature = "ssr")]
use argon2::{
    password_hash::{PasswordHash, PasswordVerifier},
    Argon2,
};

pub fn verify_password(password: &str, hash_b64: &str) -> Result<bool, String> {
    use base64::{engine::general_purpose::STANDARD as b64, Engine as _};

    log::debug!("Attempting to verify password");

    // Decode base64 hash
    let hash = String::from_utf8(
        b64.decode(hash_b64)
            .map_err(|e| format!("Failed to decode base64: {}", e))?,
    )
    .map_err(|e| format!("Failed to convert to string: {}", e))?;

    let argon2 = Argon2::default();
    let parsed_hash =
        PasswordHash::new(&hash).map_err(|e| format!("Failed to parse hash: {}", e))?;

    Ok(argon2
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok())
}
