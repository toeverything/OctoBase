use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use jwst_logger::{info, instrument, tracing};
use serde::Serialize;

pub enum ErrorStatus {
    NotModify,
    NotFound,
    NotFoundWorkspace(String),
    NotFoundInvitation,
    InternalServerError,
    PayloadTooLarge,
    BadRequest,
    Forbidden,
    Unauthorized,
    ConflictInvitation,
    PayloadExceedsLimit(String),
}

#[derive(Serialize)]
struct ErrorInfo {
    message: String,
}

#[instrument()]
fn error_response(status: StatusCode, message: &str) -> Response {
    info!("error_response enter");
    (
        status,
        Json(ErrorInfo {
            message: message.to_string(),
        }),
    )
        .into_response()
}

impl IntoResponse for ErrorStatus {
    fn into_response(self) -> Response {
        match self {
            ErrorStatus::NotModify => {
                error_response(StatusCode::NOT_MODIFIED, "The file is not modified")
            }
            ErrorStatus::NotFound => {
                error_response(StatusCode::NOT_FOUND, "The file does not exist")
            }
            ErrorStatus::NotFoundWorkspace(workspace_id) => error_response(
                StatusCode::NOT_FOUND,
                &format!("Workspace({workspace_id:?}) not found."),
            ),
            ErrorStatus::NotFoundInvitation => {
                error_response(StatusCode::NOT_FOUND, "Invitation link has expired.")
            }
            ErrorStatus::InternalServerError => error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Server error, please try again later.",
            ),
            ErrorStatus::PayloadTooLarge => error_response(
                StatusCode::PAYLOAD_TOO_LARGE,
                "Upload file size exceeds 10MB",
            ),
            ErrorStatus::BadRequest => {
                error_response(StatusCode::BAD_REQUEST, "Request parameter error.")
            }
            ErrorStatus::Forbidden => {
                error_response(StatusCode::FORBIDDEN, "Sorry, you do not have permission.")
            }
            ErrorStatus::Unauthorized => error_response(StatusCode::UNAUTHORIZED, "Unauthorized."),
            ErrorStatus::ConflictInvitation => {
                error_response(StatusCode::CONFLICT, "Invitation failed.")
            }
            ErrorStatus::PayloadExceedsLimit(limit) => error_response(
                StatusCode::PAYLOAD_TOO_LARGE,
                &format!("Upload file size exceeds {}", limit),
            ),
        }
    }
}
