use super::hosting::{rust_embed, RustEmbed};

pub const MAIL_INVITE_TITLE: &str = "{{inviter_name}} invited you to join {{workspace_name}}";
pub const MAIL_FROM: &str = "noreply@toeverything.info";
pub const MAIL_PROVIDER: &str = "smtp.gmail.com";
pub const MAIL_WHITELIST: [&str; 3] = ["affine.pro", "affine.live", "affine.systems"];

#[derive(RustEmbed)]
#[folder = "assets"]
#[include = "*"]
pub struct StaticFiles;
