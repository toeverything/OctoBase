mod blob;
mod doc;
mod error;

use async_trait::async_trait;
pub use blob::{BlobMetadata, BlobStorage, BucketBlobStorage};
pub use doc::DocStorage;
pub use error::{JwstError, JwstResult};

use super::Workspace;
