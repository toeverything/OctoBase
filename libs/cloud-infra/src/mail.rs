pub use lettre::transport::smtp::commands::Mail;

use super::*;
use chrono::prelude::*;
use cloud_database::Claims;
use handlebars::{Handlebars, RenderError};
use jwst::{warn, Base64Engine, WorkspaceMetadata, STANDARD_ENGINE};
use lettre::{
    error::Error as MailConfigError,
    message::{Mailbox, MultiPart, SinglePart},
    transport::smtp::authentication::Credentials,
    transport::smtp::Error as MailSmtpError,
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
};
use serde::Serialize;
use thiserror::Error;
use url::Url;

#[derive(Debug, Error)]
pub enum MailError {
    #[error("Mail client not initialized")]
    ClientNotInitialized,
    #[error("Failed to render mail")]
    RenderMail(#[from] RenderError),
    #[error("Failed to config mail client")]
    ConfigMail(#[from] MailConfigError),
    #[error("Failed to send email")]
    SendEmail(#[from] MailSmtpError),
}

#[derive(Serialize)]
struct MailTitle {
    inviter_name: String,
    workspace_name: String,
}

#[derive(Serialize)]
struct MailContent {
    inviter_name: String,
    site_url: String,
    avatar_url: String,
    workspace_name: String,
    invite_code: String,
    current_year: i32,
    workspace_avatar: String,
}

pub struct MailContext {
    client: Option<AsyncSmtpTransport<Tokio1Executor>>,
    mail_box: Mailbox,
    template: Handlebars<'static>,
}

impl MailContext {
    pub fn new(email: Option<String>, password: Option<String>) -> Self {
        let client = email
            .and_then(|email| password.map(|password| (email, password)))
            .map(|(email, password)| {
                let creds = Credentials::new(email, password);

                // Open a remote connection to gmail
                AsyncSmtpTransport::<Tokio1Executor>::relay(MAIL_PROVIDER)
                    .unwrap()
                    .credentials(creds)
                    .build()
            });

        if client.is_none() {
            warn!("!!! no mail account provided, email will not be sent !!!");
            warn!("!!! please set MAIL_ACCOUNT and MAIL_PASSWORD in .env file or environmental variable to enable email service !!!");
        }

        let mail_box = MAIL_FROM.parse().expect("should provide valid mail from");

        let mut template = Handlebars::new();
        template
            .register_template_string("MAIL_INVITE_TITLE", MAIL_INVITE_TITLE)
            .expect("should provide valid email title");

        let invite_file = StaticFiles::get("invite.html").unwrap();
        let invite_file = String::from_utf8_lossy(&invite_file.data);
        template
            .register_template_string("MAIL_INVITE_CONTENT", invite_file)
            .expect("should provide valid email file");

        Self {
            client,
            mail_box,
            template,
        }
    }

    pub fn parse_host(&self, host: &str) -> Option<String> {
        if let Ok(url) = Url::parse(host) {
            if let Some(host) = url.host_str() {
                if MAIL_WHITELIST.iter().any(|&domain| host.ends_with(domain)) {
                    return Some(format!("https://{host}"));
                }
            }
        }
        None
    }

    async fn make_invite_email_content(
        &self,
        metadata: WorkspaceMetadata,
        site_url: String,
        claims: &Claims,
        invite_code: &str,
        workspace_avatar: Vec<u8>,
    ) -> Result<(String, MultiPart), RenderError> {
        let base64_data = STANDARD_ENGINE.encode(workspace_avatar);
        let workspace_avatar_url = format!("data:image/jpeg;base64,{}", base64_data);
        fn string_to_color(s: &str) -> String {
            let input = if s.is_empty() { "affine" } else { s };
            let mut hash: u64 = 0;

            for char in input.chars() {
                hash = char as u64 + ((hash.wrapping_shl(5)).wrapping_sub(hash));
            }

            let mut color = String::from("#");
            for i in 0..3 {
                let value = (hash >> (i * 8)) & 0xff;
                color.push_str(&format!("{:02x}", value));
            }

            color
        }
        let workspace_avatar = if base64_data.is_empty() {
            format!(
                " <div
                style=\"
                  margin-left: 6px;
                  width: 28px;
                  height: 28px;
                  border-radius: 50%;
                  vertical-align: middle;
                  color: rgb(255, 255, 255);
                  background-color: {};
                  margin-left: 12px;
                  margin-right: 12px;
                  box-shadow: 2.8px 2.8px 4.9px rgba(58, 76, 92, 0.04),
                    -2.8px -2.8px 9.1px rgba(58, 76, 92, 0.02),
                    4.2px 4.2px 25.2px rgba(58, 76, 92, 0.06);
                \"
              >{}</div>",
                string_to_color(&metadata.name.clone().unwrap_or_default()),
                metadata
                    .name
                    .clone()
                    .unwrap_or_default()
                    .chars()
                    .next()
                    .unwrap_or_default()
            )
        } else {
            format!(
                " <img
                style=\"
                  margin-left: 6px;
                  width: 28px;
                  height: 28px;
                  border-radius: 50%;
                  vertical-align: middle;
                  margin-left: 12px;
                  margin-right: 12px;
                  box-shadow: 2.8px 2.8px 4.9px rgba(58, 76, 92, 0.04),
                    -2.8px -2.8px 9.1px rgba(58, 76, 92, 0.02),
                    4.2px 4.2px 25.2px rgba(58, 76, 92, 0.06);
                \"
                src=\"{}\"
                alt=\"\"
              />",
                workspace_avatar_url
            )
        };
        let title = self.template.render(
            "MAIL_INVITE_TITLE",
            &MailTitle {
                inviter_name: claims.user.name.clone(),
                workspace_name: metadata.name.clone().unwrap_or_default(),
            },
        )?;

        let content = self.template.render(
            "MAIL_INVITE_CONTENT",
            &MailContent {
                inviter_name: claims.user.name.clone(),
                site_url,
                avatar_url: claims.user.avatar_url.to_owned().unwrap_or("".to_string()),
                workspace_name: metadata.name.unwrap_or_default(),
                invite_code: invite_code.to_string(),
                current_year: Utc::now().year(),
                workspace_avatar,
            },
        )?;

        let msg_body = MultiPart::mixed().multipart(
            MultiPart::mixed()
                .multipart(MultiPart::related().singlepart(SinglePart::html(content))),
        );

        Ok((title, msg_body))
    }

    async fn make_invite_email(
        &self,
        send_to: Mailbox,
        metadata: WorkspaceMetadata,
        site_url: String,
        claims: &Claims,
        invite_code: &str,
        workspace_avatar: Vec<u8>,
    ) -> Result<Message, MailError> {
        let (title, msg_body) = self
            .make_invite_email_content(metadata, site_url, claims, invite_code, workspace_avatar)
            .await?;

        Ok(Message::builder()
            .from(self.mail_box.clone())
            .to(send_to)
            .subject(title)
            .multipart(msg_body)?)
    }

    pub async fn send_invite_email(
        &self,
        send_to: Mailbox,
        metadata: WorkspaceMetadata,
        site_url: String,
        claims: &Claims,
        invite_code: &str,
        workspace_avatar: Vec<u8>,
    ) -> Result<(), MailError> {
        if let Some(client) = &self.client {
            let email = self
                .make_invite_email(
                    send_to,
                    metadata,
                    site_url,
                    claims,
                    invite_code,
                    workspace_avatar,
                )
                .await?;

            let mut retry = 3;
            loop {
                match client.send(email.clone()).await {
                    Ok(_) => return Ok(()),
                    // TODO: https://github.com/lettre/lettre/issues/743
                    Err(e) if e.is_response() => {
                        if retry >= 0 {
                            retry -= 1;
                            continue;
                        } else {
                            Err(e)?
                        }
                    }
                    Err(e) => Err(e)?,
                };
            }
        } else {
            Err(MailError::ClientNotInitialized)
        }
    }
}
