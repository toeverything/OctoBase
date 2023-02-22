pub use lettre::transport::smtp::commands::Mail;

use super::constants::*;
use handlebars::Handlebars;
use lettre::{
    message::Mailbox, transport::smtp::authentication::Credentials, AsyncSmtpTransport,
    Tokio1Executor,
};

pub struct MailContext {
    pub client: AsyncSmtpTransport<Tokio1Executor>,
    pub mail_box: Mailbox,
    pub template: Handlebars<'static>,
}

impl MailContext {
    pub fn new(email: String, password: String) -> Self {
        let creds = Credentials::new(email, password);

        // Open a remote connection to gmail
        let client = AsyncSmtpTransport::<Tokio1Executor>::relay(MAIL_PROVIDER)
            .unwrap()
            .credentials(creds)
            .build();

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
}
