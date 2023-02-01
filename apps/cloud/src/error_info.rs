use axum::response::{IntoResponse, Response};
use http::StatusCode;

pub fn not_modify_error() -> Response {
    (StatusCode::NOT_MODIFIED, "The file is not modified").into_response()
}

pub fn not_found_error() -> Response {
    (StatusCode::NOT_FOUND, "The file does not exist").into_response()
}

pub fn not_found_workspace_error(workspace_id: String) -> Response {
    (
        StatusCode::NOT_FOUND,
        format!("Workspace({workspace_id:?}) not found."),
    )
        .into_response()
}

pub fn not_found_invitation_error() -> Response {
    (
        StatusCode::NOT_FOUND,
        format!("Invitation link has expired."),
    )
        .into_response()
}

pub fn internal_server_error() -> Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        "Server error, please try again later.",
    )
        .into_response()
}

pub fn payload_too_large_error() -> Response {
    (
        StatusCode::PAYLOAD_TOO_LARGE,
        "Upload file size exceeds 10MB.",
    )
        .into_response()
}

pub fn bad_request_error() -> Response {
    (StatusCode::BAD_REQUEST, "Request parameter error.").into_response()
}

pub fn forbidden_error() -> Response {
    (StatusCode::FORBIDDEN, "Sorry, you do not have permission.").into_response()
}

pub fn unauthorized_error() -> Response {
    (StatusCode::UNAUTHORIZED, "Unauthorized").into_response()
}

pub fn conflict_invitation_error() -> Response {
    (StatusCode::CONFLICT, "Invitation failed.").into_response()
}
