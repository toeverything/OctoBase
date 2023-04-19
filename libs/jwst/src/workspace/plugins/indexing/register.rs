use super::*;
use std::{
    path::PathBuf,
    rc::Rc,
    sync::{atomic::AtomicU32, Arc},
};
use tantivy::{
    query::QueryParser,
    schema::{IndexRecordOption, Schema, TextFieldIndexing, TextOptions, STORED, STRING},
    Index,
};

#[derive(Debug)]
enum IndexingStorageKind {
    /// Store index in memory (default)
    Ram,
    /// Store index in a specific directory
    #[allow(dead_code)]
    PersistedDirectory(PathBuf),
}

impl Default for IndexingStorageKind {
    fn default() -> Self {
        Self::Ram
    }
}

#[derive(Default, Debug)]
pub struct IndexingPluginRegister {
    storage_kind: IndexingStorageKind,
}

impl IndexingPluginRegister {
    #[allow(dead_code)]
    pub fn ram() -> Self {
        Self {
            storage_kind: IndexingStorageKind::Ram,
        }
    }

    #[allow(dead_code)]
    pub fn persisted_directory(path: PathBuf) -> Self {
        Self {
            storage_kind: IndexingStorageKind::PersistedDirectory(path),
        }
    }
}

impl PluginRegister for IndexingPluginRegister {
    type Plugin = IndexingPluginImpl;
    fn setup(self, ws: &mut Workspace) -> Result<IndexingPluginImpl, Box<dyn std::error::Error>> {
        let search_index = ws.metadata().search_index;
        let options = TextOptions::default().set_indexing_options(
            TextFieldIndexing::default()
                .set_tokenizer(GRAM_TOKENIZER)
                .set_index_option(IndexRecordOption::WithFreqsAndPositions),
        );

        let mut schema_builder = Schema::builder();
        schema_builder.add_text_field("block_id", STRING | STORED);
        search_index.iter().for_each(|field_name| {
            schema_builder.add_text_field(field_name.as_str(), options.clone());
        });
        let schema = schema_builder.build();

        let index_dir: Box<dyn tantivy::Directory> = match &self.storage_kind {
            IndexingStorageKind::Ram => Box::new(tantivy::directory::RamDirectory::create()),
            IndexingStorageKind::PersistedDirectory(dir) => {
                Box::new(tantivy::directory::MmapDirectory::open(dir)?)
            }
        };

        let index = Rc::new({
            let index = Index::open_or_create(index_dir, schema.clone())?;
            tokenizers_register(index.tokenizers());
            index
        });

        let mut fields = vec![];
        search_index.iter().for_each(|field_name| {
            let body = schema.get_field(field_name.as_str()).unwrap();
            fields.push(body);
        });

        let queue_reindex = Arc::new(AtomicU32::new(
            // require an initial re-index by setting the default above 0
            1,
        ));

        let queue_reindex_clone = queue_reindex.clone();
        ws.observe(move |_txn, _e| {
            // upd.update
            // let u = yrs::Update::decode_v1(&e.update).unwrap();
            // let _items = u
            //     .as_items()
            //     .into_iter()
            //     .map(|i| format!("\n  {i:?}"))
            //     .collect::<String>();
            // for item in u.as_items() {
            //     item.id;
            // }
            queue_reindex_clone.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        });

        Ok(IndexingPluginImpl {
            schema,
            query_parser: QueryParser::for_index(&index, fields),
            index,
            queue_reindex,
            search_index,
        })
    }
}
