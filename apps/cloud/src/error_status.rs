use axum::{
    response::{IntoResponse, Response},
    Json,
};
use http::StatusCode;
use serde::Serialize;
use std::io::Error;

pub enum ErrorStatus {
    NotModify,
    NotFound,
    NotFoundWorkspace(String),
    NotFoundInvitation,
    InternalServerError,
    InternalServerFileError(Error),
    PayloadTooLarge,
    BadRequest,
    Forbidden,
    Unauthorized,
    ConflictInvitation,
}

#[derive(Serialize)]
struct ErrorInfo {
    message: String,
}

fn error_response(status: StatusCode, message: &str) -> Response {
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
            ErrorStatus::NotFoundInvitation => error_response(
                StatusCode::NOT_FOUND,
                &format!("Invitation link has expired."),
            ),
            ErrorStatus::InternalServerError => error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Server error, please try again later.",
            ),
            ErrorStatus::InternalServerFileError(err) => error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                &format!("Something went wrong: {}", err),
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
        }
    }
}
