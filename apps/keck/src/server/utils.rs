use super::*;
use dashmap::mapref::entry::Entry;
use jwst::{DocStorage, Workspace};
use tokio::sync::RwLock;

pub use jwst_logger::{debug, error, info, warn};
pub use serde::{Deserialize, Serialize};
pub use uuid::Uuid;

pub async fn init_workspace<'a>(
    context: &'a Context,
    workspace: &str,
) -> Result<Arc<RwLock<Workspace>>, anyhow::Error> {
    match context.workspace.entry(workspace.to_owned()) {
        Entry::Vacant(entry) => {
            let workspace = context.storage.docs().get(workspace.into()).await?;

            Ok(entry.insert(workspace).clone())
        }
        Entry::Occupied(o) => Ok(o.into_ref().clone()),
    }
}
