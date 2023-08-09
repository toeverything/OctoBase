use super::*;
use jwst_codec::{Array, Map};

impl Space {
    pub fn to_markdown(&self) -> Option<String> {
        if let Some(title) = self.get_blocks_by_flavour("affine:page").first() {
            let mut markdown = String::new();

            if let Some(title) = title.get("title") {
                markdown.push_str(&format!("# {}", title.to_string()));
                markdown.push('\n');
            }

            for frame in title.children() {
                if let Some(frame) = self.get(&frame) {
                    let mut state = MarkdownState::default();
                    for child in frame.children() {
                        if let Some(text) = self
                            .get(&child)
                            .and_then(|child| child.to_markdown(&mut state))
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

    fn init_workspace(&mut self, meta: WorkspaceMetadata) -> JwstResult<()> {
        self.metadata.insert("name", meta.name)?;
        self.metadata.insert("avatar", meta.avatar)?;

        Ok(())
    }

    fn init_pages(&mut self) -> JwstResult<Array> {
        self.pages().or_else(|_| {
            let array = self.doc.create_array()?;
            self.metadata.insert("pages", array.clone())?;
            Ok(array)
        })
    }

    // TODO: clone from origin doc
    fn init_version(&mut self) -> JwstResult<Map> {
        self.metadata
            .get("versions")
            .and_then(|v| v.to_map())
            .ok_or(JwstError::VersionNotFound(self.id()))
            .or_else(|_| {
                let mut map = self.doc.create_map()?;
                map.insert("affine:code", 1)?;
                map.insert("affine:database", 1)?;
                map.insert("affine:divider", 1)?;
                map.insert("affine:embed", 1)?;
                map.insert("affine:frame", 1)?;
                map.insert("affine:list", 1)?;
                map.insert("affine:page", 2)?;
                map.insert("affine:paragraph", 1)?;
                map.insert("affine:surface", 1)?;

                self.metadata.insert("versions", map.clone())?;

                Ok(map)
            })
    }

    fn pages(&self) -> JwstResult<Array> {
        self.metadata
            .get("pages")
            .and_then(|pages| pages.to_array())
            .ok_or(JwstError::PageTreeNotFound(self.id()))
    }

    fn page_item(&self) -> JwstResult<Map> {
        self.pages()?
            .iter()
            .find(|page| {
                page.to_map()
                    .and_then(|page| page.get("id"))
                    .map(|id| id.to_string() == self.space_id())
                    .unwrap_or(false)
            })
            .and_then(|v| v.to_map())
            .ok_or(JwstError::PageItemNotFound(self.space_id()))
    }

    // TODO: add sub doc support
    // pub fn to_single_page(&self) -> JwstResult<Vec<u8>> {
    //     let ws = Workspace::new(self.id());
    //     let page_item = self.page_item()?;

    //     ws.with_trx(|mut t| {
    //         let space = t.get_space(self.space_id());
    //         let new_blocks = space.blocks.clone();
    //         self.blocks(|blocks| {
    //             // TODO: hacky logic for BlockSuite's special case
    //             let (roots, blocks): (Vec<_>, _) = blocks.partition(|b| {
    //                 ["affine:surface", "affine:page"].contains(&b.flavour().as_str())
    //             });

    //             for block in roots {
    //                 block.clone_block(new_blocks.clone())?;
    //             }

    //             for block in blocks {
    //                 block.clone_block(new_blocks.clone())?;
    //             }
    //             Ok::<_, JwstError>(())
    //         })?;

    //         space.init_workspace((self.metadata.clone()).into())?;
    //         space.init_version()?;

    //         let title = self
    //             .get_blocks_by_flavour("affine:page")
    //             .first()
    //             .and_then(|b| b.get("title").map(|t| t.to_string()))
    //             .unwrap_or("Untitled".into());

    //         let page_item = MapPrelim::from(HashMap::from([
    //             ("id".into(), Any::String(Box::from(self.space_id()))),
    //             (
    //                 "createDate".into(),
    //                 page_item
    //                     .get("createDate")
    //                     .map(|c| c.to_json())
    //                     .unwrap_or_else(|| Any::Number(Utc::now().timestamp_millis() as f64)),
    //             ),
    //             (
    //                 "subpageIds".into(),
    //                 Any::Array(Box::from(
    //                     page_item
    //                         .get("subpageIds")
    //                         .map(|c| c.to_json())
    //                         .and_then(|v| match v {
    //                             Any::Array(a) => Some(a.to_vec()),
    //                             _ => None,
    //                         })
    //                         .unwrap_or_default(),
    //                 )),
    //             ),
    //         ]));

    //         let page_item = space.init_pages()?.push_back(page_item)?;

    //         page_item.insert("title", TextPrelim::new(title))?;

    //         Ok::<_, JwstError>(())
    //     })?;

    //     ws.sync_migration()
    // }
}
