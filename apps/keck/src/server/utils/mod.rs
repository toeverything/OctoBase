use super::*;
use dashmap::mapref::entry::Entry;
use jwst::Workspace;
use std::sync::Arc;
use tokio::sync::{mpsc::channel, Mutex};

pub enum Migrate {
    Update(Vec<u8>),
    Full(Vec<u8>),
}

pub async fn init_doc(context: Arc<Context>, workspace: &str) {
    if let Entry::Vacant(entry) = context.workspace.entry(workspace.to_owned()) {
        let doc = context.db.create_doc(workspace).await.unwrap();
        let (tx, mut rx) = channel::<Migrate>(100);

        {
            // storage thread
            let context = context.clone();
            let workspace = workspace.to_owned();
            tokio::spawn(async move {
                while let Some(update) = rx.recv().await {
                    let res = match update {
                        Migrate::Update(update) => context.db.update(&workspace, update).await,
                        Migrate::Full(full) => context.db.full_migrate(&workspace, full).await,
                    };
                    if let Err(e) = res {
                        error!("failed to update document: {:?}", e);
                    }
                }

                info!("storage final: {}", workspace);
            });
        }

        context.storage.insert(workspace.to_owned(), tx);

        entry.insert(Mutex::new(Workspace::from_doc(doc, workspace)));
    };
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
            let mut block = t.create("test", "text");

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

        Ok(())
    }
}
