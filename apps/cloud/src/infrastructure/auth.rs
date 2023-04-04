use axum::http::HeaderMap;
use chrono::Utc;
use cloud_database::Claims;
use jsonwebtoken::{decode, DecodingKey, Validation};

pub fn get_claim_from_headers(headers: &HeaderMap, key: &DecodingKey) -> Option<Claims> {
    headers
        .get("Authorization")
        .and_then(|header| header.to_str().ok())
        .and_then(|token| get_claim_from_token(token, key))
}

pub fn get_claim_from_token(token: &str, key: &DecodingKey) -> Option<Claims> {
    decode::<Claims>(token, key, &Validation::default())
        .map(|d| d.claims)
        .ok()
        .and_then(|claims| {
            if claims.exp > Utc::now().naive_utc() {
                Some(claims)
            } else {
                None
            }
        })
}
