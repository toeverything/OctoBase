use axum::{
    response::{IntoResponse, Response},
    Json,
};
use http::StatusCode;
use serde::Serialize;

#[derive(Serialize)]
struct ErrorStatus {
    message: String,
}

fn error_response(status: StatusCode, message: &str) -> Response {
    (
        status,
        Json(ErrorStatus {
            message: message.to_string(),
        }),
    )
        .into_response()
}

pub fn not_modify_error() -> Response {
    error_response(StatusCode::NOT_MODIFIED, "The file is not modified")
}

pub fn not_found_error() -> Response {
    error_response(StatusCode::NOT_FOUND, "The file does not exist")
}

pub fn not_found_workspace_error(workspace_id: String) -> Response {
    error_response(
        StatusCode::NOT_FOUND,
        &format!("Workspace({workspace_id:?}) not found."),
    )
}

pub fn not_found_invitation_error() -> Response {
    error_response(
        StatusCode::NOT_FOUND,
        &format!("Invitation link has expired."),
    )
}

pub fn internal_server_error() -> Response {
    error_response(
        StatusCode::INTERNAL_SERVER_ERROR,
        "Server error, please try again later.",
    )
}

pub fn payload_too_large_error() -> Response {
    error_response(
        StatusCode::PAYLOAD_TOO_LARGE,
        "Upload file size exceeds 10MB",
    )
}

pub fn bad_request_error() -> Response {
    error_response(StatusCode::BAD_REQUEST, "Request parameter error.")
}

pub fn forbidden_error() -> Response {
    error_response(StatusCode::FORBIDDEN, "Sorry, you do not have permission.")
}

pub fn unauthorized_error() -> Response {
    error_response(StatusCode::UNAUTHORIZED, "Unauthorized.")
}

pub fn conflict_invitation_error() -> Response {
    error_response(StatusCode::CONFLICT, "Invitation failed.")
}
