pub use lettre::transport::smtp::commands::Mail;

use super::*;
use chrono::prelude::*;
use cloud_database::Claims;
use handlebars::{Handlebars, RenderError};
use jwst::WorkspaceMetadata;
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
    // workspace_avatar: String,
    invite_code: String,
    current_year: i32,
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
    ) -> Result<(String, MultiPart), RenderError> {
        // let mut file = ctx
        //     .storage
        //     .blobs()
        //     .get_blob(Some(workspace_id.clone()), metadata.avatar.clone().unwrap())
        //     .await
        //     .ok()?;

        // let mut file_content = Vec::new();
        // while let Some(chunk) = file.next().await {
        //     file_content.extend(chunk.ok()?);
        // }

        // let workspace_avatar = lettre::message::Body::new(file_content);

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
                // workspace_avatar: workspace_avatar.encoding().to_string(),
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
    ) -> Result<Message, MailError> {
        let (title, msg_body) = self
            .make_invite_email_content(metadata, site_url, claims, invite_code)
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
    ) -> Result<(), MailError> {
        if let Some(client) = &self.client {
            let email = self
                .make_invite_email(send_to, metadata, site_url, claims, invite_code)
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
