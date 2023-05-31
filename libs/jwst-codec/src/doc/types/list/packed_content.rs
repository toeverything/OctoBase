use super::*;

pub struct PackedContent {
    content: Vec<Any>,
    root: TypeStoreRef,
    pub(super) ref_item: Option<Box<Item>>,
    client_id: Client,
    store: DocStore,
}

impl PackedContent {
    pub(super) fn new(client_id: u64, root: TypeStoreRef, store: DocStore) -> Self {
        Self {
            content: Vec::new(),
            root,
            ref_item: None,
            client_id,
            store,
        }
    }

    pub(super) fn update_ref_item(&mut self, ref_item: Option<Box<Item>>) {
        self.ref_item = ref_item
    }

    pub(super) fn push(&mut self, content: Any) {
        self.content.push(content);
    }

    pub(super) fn build_item(&self, id: Id, cb: impl Fn(ItemBuilder) -> Item) -> StructRef {
        let mut root = self.root.borrow_mut();
        let item = cb(ItemBuilder::new()
            .id(id)
            .left_id(self.ref_item.as_ref().map(|i| i.get_last_id()))
            .right_id(
                self.ref_item
                    .as_ref()
                    .and_then(|i| i.right_id)
                    .or(root.start.as_ref().and_then(|root| root.right_id())),
            )
            .parent(Some(Parent::String(root.name.clone()))));

        let item = Box::new(item);

        let struct_ref: StructRef = StructInfo::Item { id, item }.into();

        if root.start.is_none() {
            root.start.replace(struct_ref.clone());
        }

        struct_ref
    }

    pub(super) fn pack(&mut self) -> JwstCodecResult<()> {
        if !self.content.is_empty() {
            let content = std::mem::take(&mut self.content);

            let new_id = Id::new(self.client_id, self.store.get_state(self.client_id));
            let new_struct =
                self.build_item(new_id, |b| b.content(Content::Any(content.clone())).build());

            self.store.add_item_ref(new_struct.clone())?;
            self.ref_item = new_struct.as_item();
        }
        Ok(())
    }
}
