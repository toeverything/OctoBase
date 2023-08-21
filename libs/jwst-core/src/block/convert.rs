use super::*;

const MAX_STACK: u8 = 10;

impl Block {
    pub fn to_markdown(&self, state: &mut MarkdownState) -> Option<String> {
        match self.get("text").map(|t| t.to_string()) {
            Some(text) => match self.flavour().as_str() {
                "affine:code" => {
                    state.numbered_count = 0;
                    match self.get("language").map(|v| v.to_string()).as_deref() {
                        Some(language) => Some(format!("``` {}\n{}\n```\n", language, text)),
                        None => Some(format!("```\n{}\n```\n", text)),
                    }
                }
                format @ "affine:paragraph" => {
                    state.numbered_count = 0;
                    match self.get("type").map(|v| v.to_string()).as_deref() {
                        Some(head @ "h1" | head @ "h2" | head @ "h3" | head @ "h4" | head @ "h5") => {
                            Some(format!("{} {}\n", "#".repeat(head[1..].parse().unwrap()), text))
                        }
                        Some("quote") => Some(format!("> {text}\n")),
                        Some("text") => Some(format!("{text}\n")),
                        r#type @ Some(_) | r#type @ None => {
                            if let Some(r#type) = r#type {
                                warn!("Unprocessed format: {format}, {}", r#type);
                            } else {
                                warn!("Unprocessed format: {format}");
                            }
                            Some(text)
                        }
                    }
                }
                format @ "affine:list" => match self.get("type").map(|v| v.to_string()).as_deref() {
                    Some("numbered") => {
                        state.numbered_count += 1;
                        Some(format!("{}. {text}\n", state.numbered_count))
                    }
                    Some("todo") => {
                        state.numbered_count += 1;
                        let clicked = self.get("checked").map(|v| v.to_string() == "true").unwrap_or(false);
                        Some(format!("[{}] {text}\n", if clicked { "x" } else { " " }))
                    }
                    Some("bulleted") => {
                        state.numbered_count += 1;
                        Some(format!("- {text}\n"))
                    }
                    r#type @ Some("text") | r#type @ Some(_) | r#type @ None => {
                        state.numbered_count = 0;
                        if let Some(r#type) = r#type {
                            warn!("Unprocessed format: {format}, {}", r#type);
                        } else {
                            warn!("Unprocessed format: {format}");
                        }
                        Some(text)
                    }
                },
                format => {
                    state.numbered_count = 0;
                    warn!("Unprocessed format: {format}");
                    Some(text)
                }
            },
            None => match self.flavour().as_str() {
                "affine:divider" => {
                    state.numbered_count = 0;
                    Some("---\n".into())
                }
                "affine:embed" => {
                    state.numbered_count = 0;
                    match self.get("type").map(|v| v.to_string()).as_deref() {
                        Some("image") => self
                            .get("sourceId")
                            .map(|v| format!("![](/api/workspace/{}/blob/{})\n", self.id, v.to_string())),
                        _ => None,
                    }
                }
                format => {
                    state.numbered_count = 0;
                    warn!("Unprocessed format: {format}");
                    None
                }
            },
        }
    }

    fn clone_text(&self, stack: u8, _text: Text, _new_text: Text) -> JwstResult<()> {
        if stack > MAX_STACK {
            warn!("clone_text: stack overflow");
            return Ok(());
        }

        // TODO： jwst need implement text diff
        // for Diff {
        //     insert, attributes, ..
        // } in text.diff(YChange::identity)
        // {
        //     match insert {
        //         Value::Any(Any::String(str)) => {
        //             let str = str.as_ref();
        //             if let Some(attr) = attributes {
        //                 new_text.insert_with_attributes(new_text.len(), str, *attr)?;
        //             } else {
        //                 new_text.insert(new_text.len(), str)?;
        //             }
        //         }
        //         val => {
        //             warn!("unexpected embed type: {:?}", val);
        //         }
        //     }
        // }

        Ok(())
    }

    fn clone_array(&self, stack: u8, array: Array, mut new_array: Array) -> JwstResult<()> {
        if stack > MAX_STACK {
            warn!("clone_array: stack overflow");
            return Ok(());
        }

        for item in array.iter() {
            match item {
                Value::Any(any) => {
                    new_array.push(any)?;
                }
                Value::Text(text) => {
                    let created_text = self.doc.create_text()?;
                    new_array.push(created_text.clone())?;
                    self.clone_text(stack + 1, text, created_text)?;
                }
                Value::Map(map) => {
                    let created_map = self.doc.create_map()?;
                    new_array.push(created_map.clone())?;
                    for (key, value) in map.iter() {
                        self.clone_value(stack + 1, &key, value, created_map.clone())?;
                    }
                }
                Value::Array(array) => {
                    let created_array = self.doc.create_array()?;
                    new_array.push(created_array.clone())?;
                    self.clone_array(stack + 1, array, created_array)?;
                }
                val => {
                    warn!("unexpected prop type: {:?}", val);
                }
            }
        }

        Ok(())
    }

    fn clone_value(&self, stack: u8, key: &str, value: Value, mut new_map: Map) -> JwstResult<()> {
        if stack > MAX_STACK {
            warn!("clone_value: stack overflow");
            return Ok(());
        }

        match value {
            Value::Any(any) => {
                new_map.insert(key, any)?;
            }
            Value::Text(text) => {
                let created_text = self.doc.create_text()?;
                new_map.insert(key, created_text.clone())?;
                self.clone_text(stack + 1, text, created_text)?;
            }
            Value::Map(map) => {
                let created_map = self.doc.create_map()?;
                new_map.insert(key, created_map.clone())?;
                for (key, value) in map.iter() {
                    self.clone_value(stack + 1, &key, value, created_map.clone())?;
                }
            }
            Value::Array(array) => {
                let created_array = self.doc.create_array()?;
                new_map.insert(key, created_array.clone())?;
                self.clone_array(stack + 1, array, created_array)?;
            }
            val => {
                warn!("unexpected prop type: {:?}", val);
            }
        }

        Ok(())
    }

    pub fn clone_block(&self, mut new_blocks: Map) -> JwstResult<()> {
        // init base struct
        let mut created_block = self.doc.create_map()?;
        new_blocks.insert(&*self.block_id, created_block.clone())?;

        // init default schema
        created_block.insert(sys::ID, self.block_id.clone())?;
        created_block.insert(sys::FLAVOUR, self.flavour())?;
        let mut created_children = self.doc.create_array()?;
        created_block.insert(sys::CHILDREN, created_children.clone())?;
        // created_block.insert( sys::CREATED, self.created() as f64)?;

        // clone children
        for block_id in self.children() {
            if let Err(e) = created_children.push(block_id) {
                warn!("failed to push block: {}", e);
            }
        }

        // clone props
        for key in self
            .block
            .keys()
            .iter()
            .filter(|k| k.starts_with("prop:") || k.starts_with("ext:"))
        {
            match self.block.get(key) {
                Some(value) => {
                    self.clone_value(0, key, value, created_block.clone())?;
                }
                None => {
                    warn!("failed to get key: {}", key);
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use jwst_codec::Update;

    use super::*;

    #[test]
    fn test_multiple_layer_space_clone() {
        let mut doc1 = Doc::default();
        doc1.apply_update(
            Update::from_ybinary1(include_bytes!("../../fixtures/test_multi_layer.bin").to_vec()).unwrap(),
        )
        .unwrap();

        let mut ws1 = Workspace::from_doc(doc1, "test").unwrap();

        let new_update = {
            ws1.metadata.insert("name", Some("test1")).unwrap();
            ws1.metadata.insert("avatar", Some("test2")).unwrap();
            let space = ws1.get_exists_space("page0").unwrap();
            space.to_single_page().unwrap()
        };

        let mut doc2 = Doc::default();
        doc2.apply_update(Update::from_ybinary1(new_update).unwrap()).unwrap();

        let doc1 = ws1.doc();

        assert_json_diff::assert_json_eq!(doc1.get_map("space:meta").unwrap(), doc2.get_map("space:meta").unwrap());

        // FIXME: clone block has bug that cloned but cannot store in update
        // assert_json_diff::assert_json_eq!(
        //     doc1.get_map("space:page0").unwrap(),
        //     doc2.get_map("space:page0").unwrap()
        // );
    }
}
