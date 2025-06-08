#[cfg(feature = "ssr")]
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHasher, SaltString},
    Argon2,
};
use std::env;

#[cfg(feature = "ssr")]
fn main() {
    use base64::{engine::general_purpose::STANDARD as b64, Engine as _};

    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        println!("Usage: {} <password>", args[0]);
        std::process::exit(1);
    }

    let password = &args[1];
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();

    match argon2.hash_password(password.as_bytes(), &salt) {
        Ok(hash) => {
            let hash_str = hash.to_string();
            let encoded = b64.encode(hash_str);
            println!("Base64 encoded hash:");
            println!("{}", encoded);
            println!("\nTo set in fly.io/env:");
            println!("fly secrets set ADMIN_PASSWORD_HASH='{}'", encoded);
        }
        Err(e) => {
            eprintln!("Error hashing password: {}", e);
            std::process::exit(1);
        }
    }
}

#[cfg(not(feature = "ssr"))]
fn main() {
    eprintln!("This binary requires the 'ssr' feature");
    std::process::exit(1);
}
