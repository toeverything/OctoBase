use jwst::octo::OctoPluginRegister;
use std::{
    rc::Rc,
    sync::{atomic::AtomicU32, Arc},
};
use tantivy::{
    query::QueryParser,
    schema::{IndexRecordOption, Schema, TextFieldIndexing, TextOptions, STORED, STRING},
    Index,
};

use crate::TextSearchPlugin;

mod tokenizer;

#[derive(Default, Debug)]
#[non_exhaustive] // Non-exhaustive types cannot be constructed outside of the defining crate
pub struct TextSearchPluginConfig {}
impl TextSearchPluginConfig {
    pub fn ram() -> Self {
        // already the default...
        TextSearchPluginConfig {}
    }
}

impl OctoPluginRegister for TextSearchPluginConfig {
    type Plugin = TextSearchPlugin;

    fn setup(
        self,
        workspace: &mut jwst::OctoWorkspace,
    ) -> Result<Self::Plugin, Box<dyn std::error::Error>> {
        let options = TextOptions::default().set_indexing_options(
            TextFieldIndexing::default()
                .set_tokenizer(tokenizer::LANG_CN)
                .set_index_option(IndexRecordOption::WithFreqsAndPositions),
        );

        let mut schema_builder = Schema::builder();
        schema_builder.add_text_field("block_id", STRING | STORED);
        schema_builder.add_text_field("title", options.clone()); // props:title
        schema_builder.add_text_field("body", options.clone()); // props:text
        let schema = schema_builder.build();

        // let index_dir: Box<dyn tantivy::Directory> = match &self.storage_kind {
        //     IndexingStorageKind::RAM => Box::new(tantivy::directory::RamDirectory::create()),
        //     IndexingStorageKind::PersistedDirectory(dir) => {
        //         Box::new(tantivy::directory::MmapDirectory::open(dir)?)
        //     }
        // };

        let index = Rc::new({
            let index =
                Index::open_or_create(tantivy::directory::RamDirectory::create(), schema.clone())?;
            tokenizer::tokenizers_register(index.tokenizers());
            index
        });

        let title = schema.get_field("title").unwrap();
        let body = schema.get_field("body").unwrap();

        let queue_reindex = Arc::new(AtomicU32::new(
            // require an initial re-index by setting the default above 0
            1,
        ));

        let sub = workspace.observe({
            let queue_reindex = queue_reindex.clone();
            move |_txn, _e| {
                // Future: Consider optimizing how we figure out what blocks were changed.
                // let u = yrs::Update::decode_v1(&e.update).unwrap();
                // let _items = u
                //     .as_items()
                //     .into_iter()
                //     .map(|i| format!("\n  {i:?}"))
                //     .collect::<String>();
                // for item in u.as_items() {
                //     item.id;
                // }

                // signal to the indexer that an update is needed before the next search.
                queue_reindex.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            }
        });

        Ok(TextSearchPlugin {
            tantivy_schema: schema,
            tantivy_query_parser: QueryParser::for_index(&index, vec![title, body]),
            tantivy_index: index,
            queue_reindex,
            // needs to drop sub with everything else
            _octo_update_sub: sub.expect("able to add subscription for re-indexing"),
        })
    }
}
