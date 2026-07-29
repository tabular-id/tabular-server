use axum::{
    extract::FromRequestParts,
    http::{StatusCode, request::Parts},
    RequestPartsExt,
};
use axum_extra::{TypedHeader, headers::{Authorization, authorization::Bearer}};

use crate::{AppState, auth::jwt, models::JwtClaims};

/// Extractor that validates the JWT Bearer token from the Authorization header.
/// Usage in handlers: `async fn my_handler(claims: JwtClaims, ...) -> ...`
impl FromRequestParts<AppState> for JwtClaims {
    type Rejection = (StatusCode, axum::Json<serde_json::Value>);

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let TypedHeader(Authorization(bearer)) = parts
            .extract::<TypedHeader<Authorization<Bearer>>>()
            .await
            .map_err(|_| {
                (
                    StatusCode::UNAUTHORIZED,
                    axum::Json(serde_json::json!({
                        "success": false,
                        "error": "Missing or invalid Authorization header"
                    })),
                )
            })?;

        let claims = jwt::verify_access_token(bearer.token(), &state.config.jwt_secret)
            .map_err(|_| {
                (
                    StatusCode::UNAUTHORIZED,
                    axum::Json(serde_json::json!({
                        "success": false,
                        "error": "Invalid or expired access token"
                    })),
                )
            })?;

        Ok(claims)
    }
}
