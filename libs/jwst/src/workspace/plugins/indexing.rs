use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::AtomicU32;
use std::sync::Arc;

use lib0::any::Any;
use tantivy::{collector::TopDocs, query::QueryParser, schema::*, Index, ReloadPolicy};
use yrs::updates::decoder::Decode;

use super::{Content, PluginImpl, PluginRegister, Workspace};
use serde::{Deserialize, Serialize};

/// Part of [SearchResult]
#[derive(Debug, Serialize, Deserialize)]
pub struct SearchBlockItem {
    /// Dev information. In future perhaps a way to only create this in some kind of "debug mode".
    pub debug_string: Option<String>,
    pub block_id: String,
    // Additional info not be necessary for v0, since the front-end
    // only needs to know the search specific info, which could just
    // be the Block IDs since the front-end will end up rendering the
    // blocks anyways.
    // // WIP: other tantivy info like score etc?
    // pub search_meta: HashMap<String, lib0::any::Any>,
}

/// Returned from [Workspace::search]
#[derive(Debug, Serialize, Deserialize)]
pub struct SearchBlockList {
    /// Dev information. In future perhaps a way to only create this in some kind of "debug mode".
    pub debug_string: Option<String>,
    pub items: Vec<SearchBlockItem>,
    // Additional info not be necessary for v0, since the front-end
    // only needs to know the search specific info, which could just
    // be the Block IDs since the front-end will end up rendering the
    // blocks anyways.
    // // WIP: other tantivy info like score etc?
    // pub search_meta: HashMap<String, lib0::any::Any>,
}

/// Options passed into [Workspace::search]
#[derive(Debug)]
pub struct SearchQueryOptions {
    /// Primary search text string.
    pub query: String,
}

#[derive(Default, Debug)]
pub struct IndexingPluginRegister {
    pub(self) storage_kind: IndexingStorageKind,
}

#[derive(Debug)]
pub enum IndexingStorageKind {
    /// Store index in memory (default)
    RAM,
    /// Store index in a specific directory
    #[allow(dead_code)]
    PersistedDirectory(PathBuf),
}

impl Default for IndexingStorageKind {
    fn default() -> Self {
        Self::RAM
    }
}

pub struct IndexingPluginImpl {
    // /// `true` if the text search has not yet populated the Tantivy index
    // /// `false` if there should only be incremental changes necessary to the blocks.
    // first_index: bool,
    queue_reindex: Arc<AtomicU32>,
    schema: Schema,
    index: Rc<Index>,
    query_parser: QueryParser,
    // need to keep so it gets dropped with this plugin
    _update_sub: yrs::Subscription<yrs::UpdateEvent>,
}

impl IndexingPluginImpl {
    pub fn search(
        &self,
        search_options: &SearchQueryOptions,
    ) -> Result<SearchBlockList, Box<dyn std::error::Error>> {
        let mut debug_string = format!("Searching with: {search_options:?}\n");
        let mut items = Vec::new();

        let reader = self
            .index
            .reader_builder()
            .reload_policy(ReloadPolicy::OnCommit)
            .try_into()?;
        let searcher = reader.searcher();
        let query = self.query_parser.parse_query(&search_options.query)?;
        let top_docs = searcher.search(&query, &TopDocs::with_limit(10))?;
        // The actual documents still need to be retrieved from Tantivy’s store.
        // Since the body field was not configured as stored, the document returned will only contain a title.

        if !top_docs.is_empty() {
            let block_id_field = self.schema.get_field("block_id").unwrap();

            for (search_score, doc_address) in top_docs {
                use std::fmt::Write;
                let retrieved_doc = searcher.doc(doc_address)?;
                if let Some(Value::Str(id)) = retrieved_doc.get_first(block_id_field) {
                    writeln!(&mut debug_string, "{id}")?;
                    items.push(SearchBlockItem {
                        debug_string: Some(format!("Score: {search_score}")),
                        block_id: id.to_string(),
                    })
                } else {
                    let to_json = self.schema.to_json(&retrieved_doc);
                    eprintln!(
                        "Unexpected non-block doc in Tantivy result set: {}",
                        to_json
                    )
                }
            }
        }

        Ok(SearchBlockList {
            debug_string: Some(debug_string),
            items,
        })
    }
}

impl PluginImpl for IndexingPluginImpl {
    fn on_update(&mut self, ws: &Content) -> Result<(), Box<dyn std::error::Error>> {
        let curr = self.queue_reindex.load(std::sync::atomic::Ordering::SeqCst);
        if curr > 0 {
            let mut re_index_list = HashMap::<String, (Option<String>, Option<String>)>::new();
            // TODO: reindex
            for block in ws.block_iter() {
                let title = block.content().get("title").map(ToOwned::to_owned);
                let body = block.content().get("text").map(ToOwned::to_owned);
                re_index_list.insert(
                    block.id(),
                    (
                        title.and_then(|a| match a {
                            Any::String(str) => Some(str.to_string()),
                            _ => None,
                        }),
                        body.and_then(|a| match a {
                            Any::String(str) => Some(str.to_string()),
                            _ => None,
                        }),
                    ),
                );
            }

            // dbg!((txn, upd));
            // println!("got update event: {items}");
            // just re-index stupidly
            self.re_index_content(re_index_list)
                .map_err(|err| format!("Error during reindex: {err:?}"))?;
        }

        // reset back down now that the update was applied
        self.queue_reindex
            .fetch_sub(curr, std::sync::atomic::Ordering::SeqCst);

        Ok(())
    }
}

impl PluginRegister for IndexingPluginRegister {
    type Plugin = IndexingPluginImpl;
    fn setup(self, ws: &mut Workspace) -> Result<IndexingPluginImpl, Box<dyn std::error::Error>> {
        let mut schema_builder = Schema::builder();
        schema_builder.add_text_field("block_id", STRING | STORED);
        schema_builder.add_text_field("title", TEXT); // props:title
        schema_builder.add_text_field("body", TEXT); // props:text
        let schema = schema_builder.build();

        let index_dir: Box<dyn tantivy::Directory> = match &self.storage_kind {
            IndexingStorageKind::RAM => Box::new(tantivy::directory::RamDirectory::create()),
            IndexingStorageKind::PersistedDirectory(dir) => {
                Box::new(tantivy::directory::MmapDirectory::open(dir)?)
            }
        };

        let index = Rc::new(Index::open_or_create(index_dir, schema.clone())?);
        let title = schema.get_field("title").unwrap();
        let body = schema.get_field("body").unwrap();

        let queue_reindex = Arc::new(AtomicU32::new(
            // require an initial re-index by setting the default above 0
            1,
        ));

        let sub = ws.observe({
            let queue_reindex = queue_reindex.clone();
            move |_txn, e| {
                // upd.update
                let u = yrs::Update::decode_v1(&e.update).unwrap();
                let _items = u
                    .as_items()
                    .into_iter()
                    .map(|i| format!("\n  {i:?}"))
                    .collect::<String>();
                for item in u.as_items() {
                    item.id;
                }

                queue_reindex.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            }
        });

        Ok(IndexingPluginImpl {
            schema,
            query_parser: QueryParser::for_index(&index, vec![title, body]),
            index,
            queue_reindex,
            // needs to drop sub with everything else
            _update_sub: sub,
        })
    }
}

impl IndexingPluginImpl {
    fn re_index_content<BlockIdTitleAndTextIter>(
        &mut self,
        blocks: BlockIdTitleAndTextIter,
    ) -> Result<(), Box<dyn std::error::Error>>
    where
        // TODO: use a structure with better names than tuples?
        BlockIdTitleAndTextIter: IntoIterator<Item = (String, (Option<String>, Option<String>))>,
    {
        let block_id_field = self.schema.get_field("block_id").unwrap();
        let title_field = self.schema.get_field("title").unwrap();
        let body_field = self.schema.get_field("body").unwrap();

        let mut writer = self
            .index
            .writer(50_000_000)
            .map_err(|err| format!("Error creating writer: {err:?}"))?;

        for (block_id, (block_title_opt, block_text_opt)) in blocks {
            let mut block_doc = Document::new();
            block_doc.add_text(block_id_field, block_id);
            if let Some(block_title) = block_title_opt {
                block_doc.add_text(title_field, block_title);
            }
            if let Some(block_text) = block_text_opt {
                block_doc.add_text(body_field, block_text);
            }
            writer.add_document(block_doc)?;
        }

        // If .commit() returns correctly, then all of the documents that have been added
        // are guaranteed to be persistently indexed.
        writer.commit()?;

        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::super::*;
    use super::*;

    // out of order for now, in the future, this can be made in order by sorting before
    // we reduce to just the block ids. Then maybe we could first sort on score, then sort on
    // block id.
    macro_rules! expect_result_ids {
        ($search_results:ident, $id_str_array:expr) => {
            let mut sorted_ids = $search_results
                .items
                .iter()
                .map(|i| &i.block_id)
                .collect::<Vec<_>>();
            sorted_ids.sort();
            assert_eq!(
                sorted_ids, $id_str_array,
                "Expected found ids (left) match expected ids (right) for search results"
            );
        };
    }
    macro_rules! expect_search_gives_ids {
        ($search_plugin:ident, $query_text:expr, $id_str_array:expr) => {
            let search_result = $search_plugin
                .search(&SearchQueryOptions {
                    query: $query_text.to_string(),
                })
                .expect("no error searching");

            let line = line!();
            println!("Search results (workspace.rs:{line}): {search_result:#?}"); // will show if there is an issue running the test

            expect_result_ids!(search_result, $id_str_array);
        };
    }

    #[test]
    fn basic_search_test() {
        let mut wk = {
            let mut wk = Workspace::from_doc_without_plugins(Default::default(), "wk-load");
            // even though the plugin is added by default,
            wk.setup_plugin(IndexingPluginRegister {
                storage_kind: IndexingStorageKind::RAM,
            });

            wk
        };

        wk.with_trx(|mut t| {
            let block = t.create("b1", "text");
            block.set(&mut t.trx, "test", "test");

            let block = t.create("a", "affine:text");
            let b = t.create("b", "affine:text");
            let c = t.create("c", "affine:text");
            let d = t.create("d", "affine:text");
            let e = t.create("e", "affine:text");
            let f = t.create("f", "affine:text");
            let trx = &mut t.trx;

            b.set(trx, "title", "Title B content");
            b.set(trx, "text", "Text B content bbb xxx");

            c.set(trx, "title", "Title C content");
            c.set(trx, "text", "Text C content ccc xxx yyy");

            d.set(trx, "title", "Title D content");
            d.set(trx, "text", "Text D content ddd yyy");
            // assert_eq!(b.get(trx, "title"), "Title content");
            // b.set(trx, "text", "Text content");

            // pushing blocks in
            {
                block.push_children(trx, &b);
                block.insert_children_at(trx, &c, 0);
                block.insert_children_before(trx, &d, "b");
                block.insert_children_after(trx, &e, "b");
                block.insert_children_after(trx, &f, "c");

                assert_eq!(
                    block.children(),
                    vec![
                        "c".to_owned(),
                        "f".to_owned(),
                        "d".to_owned(),
                        "b".to_owned(),
                        "e".to_owned()
                    ]
                );
            }

            // Question: Is this supposed to indicate that since this block is detached, then we should not be indexing it?
            // For example, should we walk up the parent tree to check if each block is actually attached?
            block.remove_children(trx, &d);
        });

        println!("Blocks: {:#?}", wk.blocks()); // shown if there is an issue running the test.

        wk.update_plugin::<IndexingPluginImpl>()
            .expect("update text search plugin");

        let search_plugin = wk.get_plugin::<IndexingPluginImpl>().unwrap();

        expect_search_gives_ids!(search_plugin, "content", &["b", "c", "d"]);
        expect_search_gives_ids!(search_plugin, "bbb", &["b"]);
        expect_search_gives_ids!(search_plugin, "ccc", &["c"]);
        expect_search_gives_ids!(search_plugin, "xxx", &["b", "c"]);
        expect_search_gives_ids!(search_plugin, "yyy", &["c", "d"]);
    }
}
