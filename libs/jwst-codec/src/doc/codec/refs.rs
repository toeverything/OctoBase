use super::*;

// make fields Copy + Clone without much effort
#[derive(Debug, Clone)]
#[cfg_attr(all(test, not(loom)), derive(proptest_derive::Arbitrary))]
pub(crate) enum Node {
    GC { id: Id, len: u64 },
    Skip { id: Id, len: u64 },
    Item(ItemRef),
}

impl<W: CrdtWriter> CrdtWrite<W> for Node {
    fn write(&self, writer: &mut W) -> JwstCodecResult {
        match self {
            Node::GC { len, .. } => {
                writer.write_info(0)?;
                writer.write_var_u64(*len)
            }
            Node::Skip { len, .. } => {
                writer.write_info(10)?;
                writer.write_var_u64(*len)
            }
            Node::Item(item) => item.get().unwrap().write(writer),
        }
    }
}

impl PartialEq for Node {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Node::GC { id: id1, .. }, Node::GC { id: id2, .. }) => id1 == id2,
            (Node::Skip { id: id1, .. }, Node::Skip { id: id2, .. }) => id1 == id2,
            (Node::Item(item1), Node::Item(item2)) => item1.get() == item2.get(),
            _ => false,
        }
    }
}

impl Eq for Node {
    fn assert_receiver_is_total_eq(&self) {}
}

impl From<Item> for Node {
    fn from(value: Item) -> Self {
        Self::Item(Somr::new(value))
    }
}

impl Node {
    pub fn read<R: CrdtReader>(decoder: &mut R, id: Id) -> JwstCodecResult<Self> {
        let info = decoder.read_info()?;
        let first_5_bit = info & 0b11111;

        match first_5_bit {
            0 => {
                let len = decoder.read_var_u64()?;
                Ok(Node::GC { id, len })
            }
            10 => {
                let len = decoder.read_var_u64()?;
                Ok(Node::Skip { id, len })
            }
            _ => {
                let item = Somr::new(Item::read(decoder, id, info, first_5_bit)?);

                if let Content::Type(ty) = item.get().unwrap().content.as_ref() {
                    ty.get().unwrap().write().unwrap().item = item.clone();
                }

                Ok(Node::Item(item))
            }
        }
    }

    pub fn id(&self) -> Id {
        match self {
            Node::GC { id, .. } => *id,
            Node::Skip { id, .. } => *id,
            Node::Item(item) => unsafe { item.get_unchecked() }.id,
        }
    }

    pub fn client(&self) -> Client {
        self.id().client
    }

    pub fn clock(&self) -> Clock {
        self.id().clock
    }

    pub fn len(&self) -> u64 {
        match self {
            Self::GC { len, .. } => *len,
            Self::Skip { len, .. } => *len,
            Self::Item(item) => unsafe { item.get_unchecked() }.len(),
        }
    }

    pub fn is_gc(&self) -> bool {
        matches!(self, Self::GC { .. })
    }

    pub fn is_skip(&self) -> bool {
        matches!(self, Self::Skip { .. })
    }

    pub fn is_item(&self) -> bool {
        matches!(self, Self::Item(_))
    }

    pub fn as_item(&self) -> Somr<Item> {
        if let Self::Item(item) = self {
            item.clone()
        } else {
            Somr::none()
        }
    }

    pub fn left(&self) -> Option<Self> {
        if let Node::Item(item) = self {
            item.get().and_then(|item| item.left.clone())
        } else {
            None
        }
    }

    pub fn right(&self) -> Option<Self> {
        if let Node::Item(item) = self {
            item.get().and_then(|item| item.right.clone())
        } else {
            None
        }
    }

    pub fn head(&self) -> Self {
        let mut cur = self.clone();

        while let Some(left) = cur.left() {
            if left.is_item() {
                cur = left
            } else {
                break;
            }
        }

        cur
    }

    #[allow(dead_code)]
    pub fn tail(&self) -> Self {
        let mut cur = self.clone();

        while let Some(right) = cur.right() {
            if right.is_item() {
                cur = right
            } else {
                break;
            }
        }

        cur
    }

    pub fn flags(&self) -> ItemFlags {
        if let Node::Item(item) = self {
            item.get().unwrap().flags.clone()
        } else {
            // deleted
            ItemFlags::from(4)
        }
    }

    pub fn last_id(&self) -> Id {
        let Id { client, clock } = self.id();

        Id::new(client, clock + self.len() - 1)
    }

    pub fn split_at(&self, offset: u64) -> JwstCodecResult<(Self, Self)> {
        if let Self::Item(item) = self {
            let item = item.get().unwrap();
            debug_assert!(offset > 0 && item.len() > 1 && offset < item.len());
            let id = item.id;
            let right_id = Id::new(id.client, id.clock + offset);
            let (left_content, right_content) = item.content.split(offset)?;

            let left_item = Somr::new(Item::new(
                id,
                left_content,
                // let caller connect left <-> node <-> right
                Somr::none(),
                Somr::none(),
                item.parent.clone(),
                item.parent_sub.clone(),
            ));

            let right_item = Somr::new(Item::new(
                right_id,
                right_content,
                // let caller connect left <-> node <-> right
                Somr::none(),
                Somr::none(),
                item.parent.clone(),
                item.parent_sub.clone(),
            ));

            Ok((Self::Item(left_item), Self::Item(right_item)))
        } else {
            Err(JwstCodecError::ItemSplitNotSupport)
        }
    }

    #[inline]
    #[allow(dead_code)]
    pub fn countable(&self) -> bool {
        self.flags().countable()
    }

    #[inline]
    pub fn deleted(&self) -> bool {
        self.flags().deleted()
    }
}

impl From<Option<Node>> for Somr<Item> {
    fn from(value: Option<Node>) -> Self {
        match value {
            Some(n) => n.as_item(),
            None => Somr::none(),
        }
    }
}

impl From<&Option<Node>> for Somr<Item> {
    fn from(value: &Option<Node>) -> Self {
        match value {
            Some(n) => n.as_item(),
            None => Somr::none(),
        }
    }
}

impl From<Option<&Node>> for Somr<Item> {
    fn from(value: Option<&Node>) -> Self {
        match value {
            Some(n) => n.as_item(),
            None => Somr::none(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{utils::ItemBuilder, *};
    use proptest::{collection::vec, prelude::*};

    #[test]
    fn test_struct_info() {
        loom_model!({
            {
                let struct_info = Node::GC {
                    id: Id::new(1, 0),
                    len: 10,
                };
                assert_eq!(struct_info.len(), 10);
                assert_eq!(struct_info.client(), 1);
                assert_eq!(struct_info.clock(), 0);
            }

            {
                let struct_info = Node::Skip {
                    id: Id::new(2, 0),
                    len: 20,
                };
                assert_eq!(struct_info.len(), 20);
                assert_eq!(struct_info.client(), 2);
                assert_eq!(struct_info.clock(), 0);
            }

            {
                let item = ItemBuilder::new()
                    .id((3, 0).into())
                    .left_id(None)
                    .right_id(None)
                    .parent(Some(Parent::String(String::from("parent"))))
                    .parent_sub(None)
                    .content(Content::String(String::from("content")))
                    .build();
                let struct_info = Node::Item(Somr::new(item));

                assert_eq!(struct_info.len(), 7);
                assert_eq!(struct_info.client(), 3);
                assert_eq!(struct_info.clock(), 0);
            }
        });
    }

    #[test]
    fn test_read_write_struct_info() {
        loom_model!({
            let has_not_parent_id_and_has_parent = Node::Item(Somr::new(
                ItemBuilder::new()
                    .id((0, 0).into())
                    .left_id(None)
                    .right_id(None)
                    .parent(Some(Parent::String(String::from("parent"))))
                    .parent_sub(None)
                    .content(Content::String(String::from("content")))
                    .build(),
            ));

            let has_not_parent_id_and_has_parent_with_key = Node::Item(Somr::new(
                ItemBuilder::new()
                    .id((0, 0).into())
                    .left_id(None)
                    .right_id(None)
                    .parent(Some(Parent::String(String::from("parent"))))
                    .parent_sub(Some(String::from("parent_sub")))
                    .content(Content::String(String::from("content")))
                    .build(),
            ));

            let has_parent_id = Node::Item(Somr::new(
                ItemBuilder::new()
                    .id((0, 0).into())
                    .left_id(Some((1, 2).into()))
                    .right_id(Some((2, 5).into()))
                    .parent(None)
                    .parent_sub(None)
                    .content(Content::String(String::from("content")))
                    .build(),
            ));

            let struct_infos = vec![
                Node::GC {
                    id: (0, 0).into(),
                    len: 42,
                },
                Node::Skip {
                    id: (0, 0).into(),
                    len: 314,
                },
                has_not_parent_id_and_has_parent,
                has_not_parent_id_and_has_parent_with_key,
                has_parent_id,
            ];

            for info in struct_infos {
                let mut encoder = RawEncoder::default();
                info.write(&mut encoder).unwrap();

                let mut decoder = RawDecoder::new(encoder.into_inner());
                let decoded = Node::read(&mut decoder, info.id()).unwrap();

                assert_eq!(info, decoded);
            }
        });
    }

    fn struct_info_round_trip(info: &mut Node) -> JwstCodecResult {
        if let Node::Item(item) = info {
            if let Some(item) = item.get_mut() {
                if !item.is_valid() {
                    return Ok(());
                }

                if item.content.countable() {
                    item.flags.set_countable();
                }
            }
        }
        let mut encoder = RawEncoder::default();
        info.write(&mut encoder)?;

        let ret = encoder.into_inner();
        let mut decoder = RawDecoder::new(ret);

        let decoded = Node::read(&mut decoder, info.id())?;

        assert_eq!(info, &decoded);

        Ok(())
    }

    #[cfg(not(loom))]
    proptest! {
        #[test]
        #[cfg_attr(miri, ignore)]
        fn test_random_struct_info(mut infos in vec(any::<Node>(), 0..10)) {
            for info in &mut infos {
                struct_info_round_trip(info).unwrap();
            }
        }
    }
}
