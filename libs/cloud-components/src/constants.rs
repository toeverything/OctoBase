use jwst_static::{rust_embed, RustEmbed};

pub const MAIL_INVITE_TITLE: &str = "{{inviter_name}} invited you to join {{workspace_name}}";
pub const MAIL_FROM: &str = "noreply@toeverything.info";
pub const MAIL_PROVIDER: &str = "smtp.gmail.com";

#[derive(RustEmbed)]
#[folder = "static"]
#[include = "*"]
pub struct StaticFiles;
