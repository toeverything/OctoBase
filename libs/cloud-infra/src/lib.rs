#[forbid(unsafe_code)]
mod auth;
mod constants;
mod hosting;
mod mail;

pub use auth::{FirebaseContext, KeyContext};
use constants::*;
pub use hosting::{fetch_static_response, rust_embed, with_api_doc, RustEmbed};
pub use mail::{Mail, MailContext};
use nanoid::nanoid;
