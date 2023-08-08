use base64::{
    alphabet::{STANDARD, URL_SAFE},
    engine::{general_purpose::PAD, GeneralPurpose},
};
pub use base64::{DecodeError as Base64DecodeError, Engine as Base64Engine};

pub const URL_SAFE_ENGINE: GeneralPurpose = GeneralPurpose::new(&URL_SAFE, PAD);
pub const STANDARD_ENGINE: GeneralPurpose = GeneralPurpose::new(&STANDARD, PAD);
