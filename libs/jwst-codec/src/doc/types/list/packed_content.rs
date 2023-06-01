use super::*;

pub struct PackedContent<'a> {
    client_id: Client,
    content: Vec<Any>,
    list_store: YTypeRef,
    pub(super) ref_item: Option<Arc<Item>>,
    store: &'a DocStore,
}

impl<'a> PackedContent<'a> {
    pub(super) fn new(client_id: u64, list_store: YTypeRef, store: &'a DocStore) -> Self {
        Self {
            content: Vec::new(),
            list_store,
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

    pub(super) fn build_item(&self, id: Id, cb: impl Fn(ItemBuilder) -> Item) -> StructInfo {
        let mut list_store = self.list_store.write().unwrap();

        let item = cb(ItemBuilder::new()
            .id(id)
            .left_id(self.ref_item.as_ref().map(|i| i.get_last_id()))
            .right_id(
                self.ref_item
                    .as_ref()
                    .and_then(|i| i.right_id)
                    .or(list_store.start.as_ref().and_then(|root| root.right_id())),
            )
            .parent(
                list_store
                    .root_name
                    .as_ref()
                    .map(|s| Parent::String(s.clone())),
            ));

        let item = StructInfo::Item(Arc::new(item));

        // if start item is empty, set the newly created item as start
        if list_store.start.is_none() {
            list_store.start.replace(item.clone());
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
