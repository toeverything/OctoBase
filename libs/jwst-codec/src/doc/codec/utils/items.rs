use super::*;

pub struct ItemBuilder {
    item: Item,
}

impl ItemBuilder {
    pub fn new() -> ItemBuilder {
        Self {
            item: Item::default(),
        }
    }

    pub fn id(mut self, id: Id) -> ItemBuilder {
        self.item.id = id;
        self
    }

    pub fn left_id(mut self, left_id: Option<Id>) -> ItemBuilder {
        self.item.left_id = left_id;
        self
    }

    pub fn right_id(mut self, right_id: Option<Id>) -> ItemBuilder {
        self.item.right_id = right_id;
        self
    }

    pub fn parent(mut self, parent: Option<Parent>) -> ItemBuilder {
        self.item.parent = parent;
        self
    }

    #[allow(dead_code)]
    pub fn parent_sub(mut self, parent_sub: Option<String>) -> ItemBuilder {
        self.item.parent_sub = parent_sub;
        self
    }

    pub fn content(mut self, content: Content) -> ItemBuilder {
        self.item.content = std::sync::Arc::new(content);
        self
    }

    pub fn build(self) -> Item {
        if self.item.content.countable() {
            self.item.flags.set(item_flags::ITEM_COUNTABLE);
        }

        self.item
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_item_builder() {
        let item = ItemBuilder::new()
            .id(Id::new(0, 1))
            .left_id(Some(Id::new(2, 3)))
            .right_id(Some(Id::new(4, 5)))
            .parent(Some(Parent::String("test".into())))
            .content(Content::Any(vec![Any::String("Hello".into())]))
            .build();

        assert_eq!(item.id, Id::new(0, 1));
        assert_eq!(item.left_id, Some(Id::new(2, 3)));
        assert_eq!(item.right_id, Some(Id::new(4, 5)));
        assert!(matches!(item.parent, Some(Parent::String(text)) if text == "test"));
        assert_eq!(item.parent_sub, None);
        assert_eq!(
            item.content,
            std::sync::Arc::new(Content::Any(vec![Any::String("Hello".into())]))
        );
    }
}
