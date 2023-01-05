mod history;
pub mod octo;
mod storage;
mod utils;

mod archive {
    mod block;
    mod workspace;

    // TODO: Move search results out of archive
    #[cfg(feature = "workspace-search")]
    pub use workspace::{SearchResult, SearchResults};
    pub use workspace::{Workspace, WorkspaceTransaction};
}

use std::borrow::Cow;

// re-export core types for now
pub use lib0::any::Any;

pub use history::{
    parse_history, parse_history_client, BlockHistory, HistoryOperation, RawHistory,
};
pub use log::{error, info};
pub use storage::{BlobMetadata, BlobStorage, DocStorage};
pub use utils::encode_update;

pub use octo::{
    value::OctoValue, value::OctoValueError, OctoBlockRef, OctoRead, OctoReaderForEvent,
    OctoSubscription, OctoWorkspace, OctoWorkspaceRef, OctoWrite,
};

pub(crate) mod prelude {
    //! Consider a prelude module for convenience
    pub use crate::octo::OctoBlockRef;
    pub use crate::octo::{OctoRead, OctoWrite};
    pub use crate::octo::{OctoWorkspace, OctoWorkspaceRef};
}

/// Prints both a pretty version of the error as well as the debug of the error.
/// Error tool
pub(crate) trait OctoResult<T> {
    fn octo_unwrap(self) -> T;
    /// Make into a string using debug print for convenience
    fn octo_map_err_any(self, message: impl Into<Cow<'static, str>>) -> Result<T, OctoAnyError>;
}

pub(crate) trait OctoOptionHelper<T> {
    fn octo_unwrap(self, message: impl Into<Cow<'static, str>>) -> T;
    /// Make into a string using debug print for convenience
    fn octo_ok_or_any(self, message: impl Into<Cow<'static, str>>) -> Result<T, OctoAnyError>;
}

impl<T> OctoOptionHelper<T> for Option<T> {
    #[track_caller]
    fn octo_unwrap(self, message: impl Into<Cow<'static, str>>) -> T {
        self.octo_ok_or_any(message).octo_unwrap()
    }

    fn octo_ok_or_any(self, message: impl Into<Cow<'static, str>>) -> Result<T, OctoAnyError> {
        self.ok_or_else(|| OctoAnyError {
            details: Some(Box::new(OctoExpectedSome)),
            message: message.into(),
        })
    }
}

/// Internal detail
#[derive(Debug, thiserror::Error)]
#[error("expected Some, but found None")]
struct OctoExpectedSome;

/// Simplified error
///
/// This error type is for capturing lower level details that we do
/// not have distinct errors for, yet.
///
/// Construct using [OctoResult] trait.
#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub struct OctoAnyError {
    message: std::borrow::Cow<'static, str>,
    details: Option<Box<dyn std::error::Error>>,
}

impl OctoAnyError {
    pub(crate) fn new(
        message: impl Into<std::borrow::Cow<'static, str>>,
        details: Option<Box<dyn std::error::Error>>,
    ) -> Self {
        Self {
            message: message.into(),
            details,
        }
    }

    pub(crate) fn prefix_message(self, prefix: &str) -> Self {
        let OctoAnyError { details, message } = self;
        Self {
            message: format!("{pre} {message}", pre = prefix.trim_end()).into(),
            details,
        }
    }
}

impl<T, E: std::error::Error + 'static> OctoResult<T> for Result<T, E> {
    #[track_caller]
    fn octo_unwrap(self) -> T {
        match self {
            Ok(value) => value,
            Err(err) => {
                if cfg!(debug_assertions) {
                    let indented = format!("{err:#?}")
                        .lines()
                        .flat_map(|ln| ["\n  ", ln])
                        .collect::<String>();
                    eprintln!("Octo usage error: {err}\nDetails:\n{indented}");
                }
                panic!("Octo usage error: {err}; Details: {err:?}")
            }
        }
    }

    fn octo_map_err_any(self, message: impl Into<Cow<'static, str>>) -> Result<T, OctoAnyError> {
        self.map_err(|err| OctoAnyError::new(message, Some(Box::new(err))))
    }
}
