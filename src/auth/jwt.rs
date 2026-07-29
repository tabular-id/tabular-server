use chrono::Utc;
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};

use crate::models::JwtClaims;

pub fn generate_access_token(
    user_id: &str,
    email: &str,
    secret: &str,
    expiry_minutes: i64,
) -> crate::error::Result<String> {
    let now = Utc::now().timestamp() as usize;
    let exp = (Utc::now() + chrono::Duration::minutes(expiry_minutes)).timestamp() as usize;

    let claims = JwtClaims {
        sub: user_id.to_string(),
        email: email.to_string(),
        iat: now,
        exp,
    };

    let token = encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )?;
    Ok(token)
}

pub fn verify_access_token(token: &str, secret: &str) -> crate::error::Result<JwtClaims> {
    let validation = Validation::new(Algorithm::HS256);
    let token_data = decode::<JwtClaims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &validation,
    )?;
    Ok(token_data.claims)
}

pub fn generate_refresh_token() -> String {
    let bytes: Vec<u8> = (0..64).map(|_| rand::random::<u8>()).collect();
    hex::encode(bytes)
}
