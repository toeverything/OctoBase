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

    pub fn parent_sub(mut self, parent_sub: Option<String>) -> ItemBuilder {
        self.item.parent_sub = parent_sub;
        self
    }

    pub fn content(mut self, content: Content) -> ItemBuilder {
        self.item.content = content;
        self
    }

    pub fn build(self) -> Item {
        self.item
    }
}
