use super::*;

const MAX_STACK: u8 = 10;

impl Block {
    pub fn to_markdown<T>(&self, trx: &T, state: &mut MarkdownState) -> Option<String>
    where
        T: ReadTxn,
    {
        match self.get(trx, "text").map(|t| t.to_string()) {
            Some(text) => match self.flavour(trx).as_str() {
                "affine:code" => {
                    state.numbered_count = 0;
                    match self.get(trx, "language").map(|v| v.to_string()).as_deref() {
                        Some(language) => Some(format!("``` {}\n{}\n```\n", language, text)),
                        None => Some(format!("```\n{}\n```\n", text)),
                    }
                }
                format @ "affine:paragraph" => {
                    state.numbered_count = 0;
                    match self.get(trx, "type").map(|v| v.to_string()).as_deref() {
                        Some(
                            head @ "h1" | head @ "h2" | head @ "h3" | head @ "h4" | head @ "h5",
                        ) => Some(format!(
                            "{} {}\n",
                            "#".repeat(head[1..].parse().unwrap()),
                            text
                        )),
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
                format @ "affine:list" => {
                    match self.get(trx, "type").map(|v| v.to_string()).as_deref() {
                        Some("numbered") => {
                            state.numbered_count += 1;
                            Some(format!("{}. {text}\n", state.numbered_count))
                        }
                        Some("todo") => {
                            state.numbered_count += 1;
                            let clicked = self
                                .get(trx, "checked")
                                .map(|v| v.to_string() == "true")
                                .unwrap_or(false);
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
                    }
                }
                format => {
                    state.numbered_count = 0;
                    warn!("Unprocessed format: {format}");
                    Some(text)
                }
            },
            None => match self.flavour(trx).as_str() {
                "affine:divider" => {
                    state.numbered_count = 0;
                    Some("---\n".into())
                }
                "affine:embed" => {
                    state.numbered_count = 0;
                    match self.get(trx, "type").map(|v| v.to_string()).as_deref() {
                        Some("image") => self
                            .get(trx, "sourceId")
                            .map(|v| format!("![](/api/workspace/{}/blob/{})\n", self.id, v)),
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

    fn clone_text<T>(
        &self,
        stack: u8,
        trx: &T,
        text: TextRef,
        new_trx: &mut TransactionMut,
        new_text: TextRef,
    ) -> JwstResult<()>
    where
        T: ReadTxn,
    {
        if stack > MAX_STACK {
            warn!("clone_text: stack overflow");
            return Ok(());
        }

        for Diff {
            insert, attributes, ..
        } in text.diff(trx, YChange::identity)
        {
            match insert {
                Value::Any(Any::String(str)) => {
                    let str = str.as_ref();
                    if let Some(attr) = attributes {
                        new_text.insert_with_attributes(
                            new_trx,
                            new_text.len(new_trx),
                            str,
                            *attr,
                        )?;
                    } else {
                        new_text.insert(new_trx, new_text.len(new_trx), str)?;
                    }
                }
                val => {
                    warn!("unexpected embed type: {:?}", val);
                }
            }
        }

        Ok(())
    }

    fn clone_array<T>(
        &self,
        stack: u8,
        trx: &T,
        array: ArrayRef,
        new_trx: &mut TransactionMut,
        new_array: ArrayRef,
    ) -> JwstResult<()>
    where
        T: ReadTxn,
    {
        if stack > MAX_STACK {
            warn!("clone_array: stack overflow");
            return Ok(());
        }

        for item in array.iter(trx) {
            match item {
                Value::Any(any) => {
                    new_array.push_back(new_trx, any)?;
                }
                Value::YText(text) => {
                    let new_text = new_array.push_back(new_trx, TextPrelim::new(""))?;
                    self.clone_text(stack + 1, trx, text, new_trx, new_text)?;
                }
                Value::YMap(map) => {
                    let new_map = new_array.push_back(new_trx, MapPrelim::<Any>::new())?;
                    for (key, value) in map.iter(trx) {
                        self.clone_value(stack + 1, key, trx, value, new_trx, new_map.clone())?;
                    }
                }
                Value::YArray(array) => {
                    let new_array = new_array.push_back(new_trx, ArrayPrelim::default())?;
                    self.clone_array(stack + 1, trx, array, new_trx, new_array)?;
                }
                val => {
                    warn!("unexpected prop type: {:?}", val);
                }
            }
        }

        Ok(())
    }

    fn clone_value<T>(
        &self,
        stack: u8,
        key: &str,
        trx: &T,
        value: Value,
        new_trx: &mut TransactionMut,
        new_map: MapRef,
    ) -> JwstResult<()>
    where
        T: ReadTxn,
    {
        if stack > MAX_STACK {
            warn!("clone_value: stack overflow");
            return Ok(());
        }

        match value {
            Value::Any(any) => {
                new_map.insert(new_trx, key, any)?;
            }
            Value::YText(text) => {
                let new_text = new_map.insert(new_trx, key, TextPrelim::new(""))?;
                self.clone_text(stack + 1, trx, text, new_trx, new_text)?;
            }
            Value::YMap(map) => {
                let new_map = new_map.insert(new_trx, key, MapPrelim::<Any>::new())?;
                for (key, value) in map.iter(trx) {
                    self.clone_value(stack + 1, key, trx, value, new_trx, new_map.clone())?;
                }
            }
            Value::YArray(array) => {
                let new_array = new_map.insert(new_trx, key, ArrayPrelim::default())?;
                self.clone_array(stack + 1, trx, array, new_trx, new_array)?;
            }
            val => {
                warn!("unexpected prop type: {:?}", val);
            }
        }

        Ok(())
    }

    pub fn clone_block<T>(
        &self,
        orig_trx: &T,
        new_trx: &mut TransactionMut,
        new_blocks: MapRef,
    ) -> JwstResult<()>
    where
        T: ReadTxn,
    {
        // init base struct
        let block = new_blocks.insert(new_trx, &*self.block_id, MapPrelim::<Any>::new())?;

        // init default schema
        block.insert(new_trx, sys::ID, self.block_id.as_ref())?;
        block.insert(new_trx, sys::FLAVOUR, self.flavour(orig_trx).as_ref())?;
        let children = block.insert(
            new_trx,
            sys::CHILDREN,
            ArrayPrelim::<Vec<String>, String>::from(vec![]),
        )?;
        // block.insert(new_trx, sys::CREATED, self.created(orig_trx) as f64)?;

        // clone children
        for block_id in self.children(orig_trx) {
            if let Err(e) = children.push_back(new_trx, block_id) {
                warn!("failed to push block: {}", e);
            }
        }

        // clone props
        for key in self
            .block
            .keys(orig_trx)
            .filter(|k| k.starts_with("prop:") || k.starts_with("ext:"))
        {
            match self.block.get(orig_trx, key) {
                Some(value) => {
                    self.clone_value(0, key, orig_trx, value, new_trx, block.clone())?;
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
    use super::*;
    use yrs::{updates::decoder::Decode, Update};

    #[test]
    fn test_multiple_layer_space_clone() {
        let doc1 = Doc::new();
        doc1.transact_mut().apply_update(
            Update::decode_v1(include_bytes!("../../fixtures/test_multi_layer.bin")).unwrap(),
        );

        let ws1 = Workspace::from_doc(doc1, "test");

        let new_update = ws1.with_trx(|mut t| {
            ws1.metadata
                .insert(&mut t.trx, "name", Some("test1"))
                .unwrap();
            ws1.metadata
                .insert(&mut t.trx, "avatar", Some("test2"))
                .unwrap();
            let space = t.get_exists_space("page0").unwrap();
            space.to_single_page(&t.trx).unwrap()
        });

        let doc2 = Doc::new();
        doc2.transact_mut()
            .apply_update(Update::decode_v1(&new_update).unwrap());

        let doc1 = ws1.doc();
        let doc1_trx = doc1.transact();
        let doc2_trx = doc2.transact();
        assert_json_diff::assert_json_eq!(
            doc1_trx.get_map("space:meta").unwrap().to_json(&doc1_trx),
            doc2_trx.get_map("space:meta").unwrap().to_json(&doc2_trx)
        );
        assert_json_diff::assert_json_eq!(
            doc1_trx.get_map("space:page0").unwrap().to_json(&doc1_trx),
            doc2_trx.get_map("space:page0").unwrap().to_json(&doc2_trx)
        );
    }
}
