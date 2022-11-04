use super::*;
use dashmap::mapref::entry::Entry;
use jwst::Workspace;
use sqlx::Error;
use std::sync::Arc;
use tokio::sync::Mutex;

pub use jwst_logger::{debug, error, info, warn};
pub use serde::{Deserialize, Serialize};
pub use uuid::Uuid;

pub async fn init_doc(context: Arc<Context>, workspace: &str) -> Result<(), Error> {
    if let Entry::Vacant(entry) = context.workspace.entry(workspace.to_owned()) {
        let doc = context.db.create_doc(workspace).await?;

        entry.insert(Mutex::new(Workspace::from_doc(doc, workspace)));
    };

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn doc_load_test() -> anyhow::Result<()> {
        use jwst::Workspace;
        use yrs::{updates::decoder::Decode, Doc, StateVector, Update};

        let workspace = Workspace::new("test");
        workspace.with_trx(|mut t| {
            let block = t.create("test", "text");

            block.set(&mut t.trx, "test", "test");
        });

        let doc = workspace.doc();

        let new_doc = {
            let update = doc.encode_state_as_update_v1(&StateVector::default());
            let doc = Doc::default();
            let mut trx = doc.transact();
            match Update::decode_v1(&update) {
                Ok(update) => trx.apply_update(update),
                Err(err) => info!("failed to decode update: {:?}", err),
            }
            trx.commit();
            doc
        };

        assert_json_diff::assert_json_eq!(
            doc.transact().get_map("blocks").to_json(),
            new_doc.transact().get_map("blocks").to_json()
        );

        assert_json_diff::assert_json_eq!(
            doc.transact().get_map("updated").to_json(),
            new_doc.transact().get_map("updated").to_json()
        );

        Ok(())
    }
}
