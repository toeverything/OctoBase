use super::*;

pub struct PackedContent<'a> {
    content: Vec<Any>,
    root: Option<StructInfo>,
    root_name: Option<String>,
    pub(super) ref_item: Option<Arc<Item>>,
    client_id: Client,
    store: &'a DocStore,
}

impl<'a> PackedContent<'a> {
    pub(super) fn new(
        client_id: u64,
        root: Option<StructInfo>,
        root_name: Option<String>,
        store: &'a DocStore,
    ) -> Self {
        Self {
            content: Vec::new(),
            root,
            root_name,
            ref_item: None,
            client_id,
            store,
        }
    }

    pub(super) fn update_ref_item(&mut self, ref_item: Option<Arc<Item>>) {
        self.ref_item = ref_item
    }

    pub(super) fn push(&mut self, content: Any) {
        self.content.push(content);
    }

    pub(super) fn build_item(&mut self, id: Id, cb: impl Fn(ItemBuilder) -> Item) -> StructInfo {
        let item = cb(ItemBuilder::new()
            .id(id)
            .left_id(self.ref_item.as_ref().map(|i| i.get_last_id()))
            .right_id(
                self.ref_item
                    .as_ref()
                    .and_then(|i| i.right_id)
                    .or(self.root.as_ref().and_then(|root| root.right_id())),
            )
            .parent(self.root_name.as_ref().map(|s| Parent::String(s.clone()))));

        let item = StructInfo::Item(Arc::new(item));
        if self.root.is_none() {
            self.root.replace(item.clone());
        }
        item
    }

    pub(super) fn pack(&mut self) -> JwstCodecResult<()> {
        if !self.content.is_empty() {
            let content = std::mem::take(&mut self.content);

            let new_id = Id::new(self.client_id, self.store.get_state(self.client_id));
            let new_struct =
                self.build_item(new_id, |b| b.content(Content::Any(content.clone())).build());

            self.store.add_item(new_struct.clone())?;
            self.ref_item = new_struct.as_item();
        }
        Ok(())
    }
}
