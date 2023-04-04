use super::*;

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

    fn clone_value<T>(
        &self,
        key: &str,
        orig_trx: &T,
        orig_value: Value,
        new_trx: &mut TransactionMut,
        new_map: MapRef,
    ) -> JwstResult<()>
    where
        T: ReadTxn,
    {
        match orig_value {
            Value::Any(any) => {
                new_map.insert(new_trx, key, any)?;
            }
            Value::YText(text) => {
                let new_text = new_map.insert(new_trx, key, TextPrelim::new(""))?;
                for Diff {
                    insert, attributes, ..
                } in text.diff(orig_trx, YChange::identity)
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
            }
            Value::YMap(map) => {
                let new_map = new_map.insert(new_trx, key, MapPrelim::<Any>::new())?;
                for (key, value) in map.iter(orig_trx) {
                    self.clone_value(key, orig_trx, value, new_trx, new_map)?;
                }
            }
            Value::YArray(array) => {
                let new_array = new_map.insert(new_trx, key, ArrayPrelim::default())?;
                for value in array.iter(orig_trx) {
                    self.clone_value(&index.to_string(), orig_trx, value, new_trx, new_array)?;
                }
            }
            val => {
                warn!("unexpected prop type: {:?}", val);
            }
        }

        Ok(())
    }

    // fn clone_map<T>(
    //     &self,
    //     orig_trx: &T,
    //     orig_map: MapRef,
    //     new_trx: &mut TransactionMut,
    //     new_map: MapRef,
    // ) -> JwstResult<()>
    // where
    //     T: ReadTxn,
    // {
    //     for key in block.keys(orig_trx) {

    //     }
    // }

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
        block.insert(new_trx, sys::VERSION, ArrayPrelim::from([1, 0]))?;
        let children = block.insert(
            new_trx,
            sys::CHILDREN,
            ArrayPrelim::<Vec<String>, String>::from(vec![]),
        )?;
        block.insert(new_trx, sys::CREATED, self.created(orig_trx) as f64)?;

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
                    self.clone_value(key, orig_trx, value, new_trx, self.block.clone())?;
                }
                None => {
                    warn!("failed to get key: {}", key);
                }
            }
        }

        Ok(())
    }
}
