#![allow(dead_code)]
use super::*;
use crate::sync::Arc;

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

    pub fn left(mut self, left: Option<StructInfo>) -> ItemBuilder {
        let origin_id = left.as_ref().map(|i| i.last_id());
        self.item.left = left.map(|i| i.as_weak());
        self.item.origin_left_id = origin_id;
        self
    }

    pub fn right(mut self, right: Option<StructInfo>) -> ItemBuilder {
        let origin_id = right.as_ref().map(|i| i.id());
        self.item.right = right.map(|i| i.as_weak());
        self.item.origin_right_id = origin_id;
        self
    }

    pub fn left_id(mut self, left_id: Option<Id>) -> ItemBuilder {
        self.item.origin_left_id = left_id;
        self
    }

    pub fn right_id(mut self, right_id: Option<Id>) -> ItemBuilder {
        self.item.origin_right_id = right_id;
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
        self.item.content = Arc::new(content);
        self
    }

    pub fn flags(mut self, flags: ItemFlags) -> ItemBuilder {
        self.item.flags = flags;
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
        loom_model!({
            let item = ItemBuilder::new()
                .id(Id::new(0, 1))
                .left_id(Some(Id::new(2, 3)))
                .right_id(Some(Id::new(4, 5)))
                .parent(Some(Parent::String("test".into())))
                .content(Content::Any(vec![Any::String("Hello".into())]))
                .build();

            assert_eq!(item.id, Id::new(0, 1));
            assert_eq!(item.origin_left_id, Some(Id::new(2, 3)));
            assert_eq!(item.origin_right_id, Some(Id::new(4, 5)));
            assert!(matches!(item.parent, Some(Parent::String(text)) if text == "test"));
            assert_eq!(item.parent_sub, None);
            assert_eq!(
                item.content,
                Arc::new(Content::Any(vec![Any::String("Hello".into())]))
            );
        });
    }
}
