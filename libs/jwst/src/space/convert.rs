use std::collections::HashMap;

use super::*;
use chrono::Utc;
use lib0::any::Any;
use yrs::{types::ToJson, Array, ArrayPrelim, ArrayRef, Map, MapPrelim, TextPrelim};

impl Space {
    pub fn to_markdown<T>(&self, trx: &T) -> Option<String>
    where
        T: ReadTxn,
    {
        if let Some(title) = self.get_blocks_by_flavour(trx, "affine:page").first() {
            let mut markdown = String::new();

            if let Some(title) = title.get(trx, "title") {
                markdown.push_str(&format!("# {title}"));
                markdown.push('\n');
            }

            for frame in title.children(trx) {
                if let Some(frame) = self.get(trx, &frame) {
                    let mut state = MarkdownState::default();
                    for child in frame.children(trx) {
                        if let Some(text) = self
                            .get(trx, &child)
                            .and_then(|child| child.to_markdown(trx, &mut state))
                        {
                            markdown.push_str(&text);
                            markdown.push('\n');
                        }
                    }
                }
            }

            Some(markdown)
        } else {
            None
        }
    }

    fn init_workspace(&self, trx: &mut TransactionMut, meta: WorkspaceMetadata) -> JwstResult<()> {
        self.metadata.insert(trx, "name", meta.name)?;
        self.metadata.insert(trx, "avatar", meta.avatar)?;

        Ok(())
    }

    fn init_pages(&self, trx: &mut TransactionMut) -> JwstResult<ArrayRef> {
        self.pages(trx)
            .or_else(|_| Ok(self.metadata.insert(trx, "pages", ArrayPrelim::default())?))
    }

    // TODO: clone from origin doc
    fn init_version(&self, trx: &mut TransactionMut) -> JwstResult<MapRef> {
        self.metadata
            .get(trx, "versions")
            .and_then(|v| v.to_ymap())
            .ok_or(JwstError::VersionNotFound(self.id()))
            .or_else(|_| {
                Ok(self.metadata.insert(
                    trx,
                    "versions",
                    MapPrelim::<Any>::from([
                        ("affine:code".to_owned(), 1.into()),
                        ("affine:database".to_owned(), 1.into()),
                        ("affine:divider".to_owned(), 1.into()),
                        ("affine:embed".to_owned(), 1.into()),
                        ("affine:frame".to_owned(), 1.into()),
                        ("affine:list".to_owned(), 1.into()),
                        ("affine:page".to_owned(), 2.into()),
                        ("affine:paragraph".to_owned(), 1.into()),
                        ("affine:surface".to_owned(), 1.into()),
                    ]),
                )?)
            })
    }

    fn pages<T: ReadTxn>(&self, trx: &T) -> JwstResult<ArrayRef> {
        self.metadata
            .get(trx, "pages")
            .and_then(|pages| pages.to_yarray())
            .ok_or(JwstError::PageTreeNotFound(self.id()))
    }

    fn page_item<T: ReadTxn>(&self, trx: &T) -> JwstResult<MapRef> {
        self.pages(trx)?
            .iter(trx)
            .find(|page| {
                page.clone()
                    .to_ymap()
                    .and_then(|page| page.get(trx, "id"))
                    .map(|id| id.to_string(trx) == self.space_id())
                    .unwrap_or(false)
            })
            .and_then(|v| v.to_ymap())
            .ok_or(JwstError::PageItemNotFound(self.space_id()))
    }

    pub fn to_single_page<T: ReadTxn>(&self, trx: &T) -> JwstResult<Vec<u8>> {
        let ws = Workspace::new(self.id());
        let page_item = self.page_item(trx)?;

        ws.with_trx(|mut t| {
            let space = t.get_space(self.space_id());
            let new_blocks = space.blocks.clone();
            self.blocks(trx, |blocks| {
                // TODO: hacky logic for BlockSuite's special case
                let (roots, blocks): (Vec<_>, _) = blocks.partition(|b| {
                    ["affine:surface", "affine:page"].contains(&b.flavour(trx).as_str())
                });

                for block in roots {
                    block.clone_block(trx, &mut t.trx, new_blocks.clone())?;
                }

                for block in blocks {
                    block.clone_block(trx, &mut t.trx, new_blocks.clone())?;
                }
                Ok::<_, JwstError>(())
            })?;

            space.init_workspace(&mut t.trx, (trx, self.metadata.clone()).into())?;
            space.init_version(&mut t.trx)?;

            let title = self
                .get_blocks_by_flavour(trx, "affine:page")
                .first()
                .and_then(|b| b.get(trx, "title").map(|t| t.to_string()))
                .unwrap_or("Untitled".into());

            let page_item = MapPrelim::from(HashMap::from([
                ("id".into(), Any::String(Box::from(self.space_id()))),
                (
                    "createDate".into(),
                    page_item
                        .get(trx, "createDate")
                        .map(|c| c.to_json(trx))
                        .unwrap_or_else(|| Any::Number(Utc::now().timestamp_millis() as f64)),
                ),
                (
                    "subpageIds".into(),
                    Any::Array(Box::from(
                        page_item
                            .get(trx, "subpageIds")
                            .map(|c| c.to_json(trx))
                            .and_then(|v| match v {
                                Any::Array(a) => Some(a.to_vec()),
                                _ => None,
                            })
                            .unwrap_or_default(),
                    )),
                ),
            ]));

            let page_item = space
                .init_pages(&mut t.trx)?
                .push_back(&mut t.trx, page_item)?;

            page_item.insert(&mut t.trx, "title", TextPrelim::new(title))?;

            Ok::<_, JwstError>(())
        })?;

        ws.sync_migration(10)
    }
}
