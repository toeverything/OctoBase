use super::*;
use crate::sync::{Arc, Weak};

// make fields Copy + Clone without much effort
#[derive(Debug)]
#[cfg_attr(all(test, not(loom)), derive(proptest_derive::Arbitrary))]
pub enum StructInfo {
    GC {
        id: Id,
        len: u64,
    },
    Skip {
        id: Id,
        len: u64,
    },
    Item(ItemRef),
    #[cfg_attr(all(test, not(loom)), proptest(skip))]
    WeakItem(Weak<Item>),
}

impl Clone for StructInfo {
    fn clone(&self) -> Self {
        match self {
            Self::GC { id, len } => Self::GC { id: *id, len: *len },
            Self::Skip { id, len } => Self::Skip { id: *id, len: *len },
            Self::Item(item) => Self::Item(item.clone()),
            Self::WeakItem(item) => Self::WeakItem(item.clone()),
        }
    }
}

impl<W: CrdtWriter> CrdtWrite<W> for StructInfo {
    fn write(&self, writer: &mut W) -> JwstCodecResult {
        match self {
            StructInfo::GC { len, .. } => {
                writer.write_info(0)?;
                writer.write_var_u64(*len)
            }
            StructInfo::Skip { len, .. } => {
                writer.write_info(10)?;
                writer.write_var_u64(*len)
            }
            StructInfo::Item(item) => item.write(writer),
            StructInfo::WeakItem(item) => {
                let item = item.upgrade().unwrap();
                item.write(writer)
            }
        }
    }
}

impl PartialEq for StructInfo {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (StructInfo::GC { id: id1, .. }, StructInfo::GC { id: id2, .. }) => id1 == id2,
            (StructInfo::Skip { id: id1, .. }, StructInfo::Skip { id: id2, .. }) => id1 == id2,
            (StructInfo::Item(item1), StructInfo::Item(item2)) => item1.id == item2.id,
            _ => false,
        }
    }
}

impl Eq for StructInfo {
    fn assert_receiver_is_total_eq(&self) {}
}

impl From<Item> for StructInfo {
    fn from(value: Item) -> Self {
        Self::Item(Arc::new(value))
    }
}

impl StructInfo {
    pub fn read<R: CrdtReader>(decoder: &mut R, id: Id) -> JwstCodecResult<Self> {
        let info = decoder.read_info()?;
        let first_5_bit = info & 0b11111;

        match first_5_bit {
            0 => {
                let len = decoder.read_var_u64()?;
                Ok(StructInfo::GC { id, len })
            }
            10 => {
                let len = decoder.read_var_u64()?;
                Ok(StructInfo::Skip { id, len })
            }
            _ => {
                let item = Arc::new(Item::read(decoder, id, info, first_5_bit)?);

                match item.content.as_ref() {
                    Content::Type(ty) => {
                        ty.write().unwrap().item = Some(Arc::downgrade(&item));
                    }
                    Content::WeakType(ty) => {
                        ty.upgrade().unwrap().write().unwrap().item = Some(Arc::downgrade(&item));
                    }
                    _ => {}
                }

                Ok(StructInfo::Item(item))
            }
        }
    }

    pub fn id(&self) -> Id {
        match self {
            StructInfo::GC { id, .. } => *id,
            StructInfo::Skip { id, .. } => *id,
            StructInfo::Item(item) => item.id,
            StructInfo::WeakItem(item) => item.upgrade().unwrap().id,
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
            Self::Item(item) => item.len(),
            Self::WeakItem(item) => item.upgrade().unwrap().len(),
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

    pub fn as_item(&self) -> Option<Arc<Item>> {
        if let Self::Item(item) = self {
            Some(item.clone())
        } else if let Self::WeakItem(item) = self {
            item.upgrade()
        } else {
            None
        }
    }

    pub fn as_weak_item(&self) -> Option<Weak<Item>> {
        if let Self::Item(item) = self {
            Some(Arc::downgrade(item))
        } else if let Self::WeakItem(item) = self {
            Some(item.clone())
        } else {
            None
        }
    }

    pub fn as_strong(&self) -> Option<Self> {
        if let Self::WeakItem(item) = self {
            item.upgrade().map(Self::Item)
        } else {
            Some(self.clone())
        }
    }

    pub fn as_weak(&self) -> Self {
        if let Self::Item(item) = self {
            Self::WeakItem(Arc::downgrade(item))
        } else {
            self.clone()
        }
    }

    pub fn left(&self) -> Option<Self> {
        if let StructInfo::Item(item) = self {
            item.left.clone()
        } else if let StructInfo::WeakItem(item) = self {
            item.upgrade().and_then(|i| i.left.clone())
        } else {
            None
        }
    }

    pub fn right(&self) -> Option<Self> {
        if let StructInfo::Item(item) = self {
            item.right.clone()
        } else if let StructInfo::WeakItem(item) = self {
            item.upgrade().and_then(|i| i.right.clone())
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
        if let StructInfo::Item(item) = self {
            item.flags.clone()
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
        if let Some(Self::Item(item)) = self.as_strong() {
            debug_assert!(offset > 0 && item.len() > 1 && offset < item.len());
            let id = item.id;
            let right_id = Id::new(id.client, id.clock + offset);
            let (left_content, right_content) = item.content.split(offset)?;

            let left_item = Arc::new(Item::new(
                id,
                left_content,
                // let caller connect left <-> node <-> right
                None,
                None,
                item.parent.clone(),
                item.parent_sub.clone(),
            ));

            let right_item = Arc::new(Item::new(
                right_id,
                right_content,
                // let caller connect left <-> node <-> right
                None,
                None,
                item.parent.clone(),
                item.parent_sub.clone(),
            ));

            Ok((Self::Item(left_item), Self::Item(right_item)))
        } else {
            Err(JwstCodecError::ItemSplitNotSupport)
        }
    }

    pub fn delete(&self) {
        if let StructInfo::Item(item) = self {
            item.delete()
        }
    }

    #[allow(dead_code)]
    pub(crate) fn countable(&self) -> bool {
        if let StructInfo::Item(item) = self {
            item.flags.countable()
        } else {
            false
        }
    }

    pub(crate) fn deleted(&self) -> bool {
        if let StructInfo::Item(item) = self {
            item.deleted()
        } else {
            false
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
                let struct_info = StructInfo::GC {
                    id: Id::new(1, 0),
                    len: 10,
                };
                assert_eq!(struct_info.len(), 10);
                assert_eq!(struct_info.client(), 1);
                assert_eq!(struct_info.clock(), 0);
            }

            {
                let struct_info = StructInfo::Skip {
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
                let struct_info = StructInfo::Item(Arc::new(item));

                assert_eq!(struct_info.len(), 7);
                assert_eq!(struct_info.client(), 3);
                assert_eq!(struct_info.clock(), 0);
            }
        });
    }

    #[test]
    fn test_read_write_struct_info() {
        loom_model!({
            let has_not_parent_id_and_has_parent = StructInfo::Item(Arc::new(
                ItemBuilder::new()
                    .id((0, 0).into())
                    .left_id(None)
                    .right_id(None)
                    .parent(Some(Parent::String(String::from("parent"))))
                    .parent_sub(None)
                    .content(Content::String(String::from("content")))
                    .build(),
            ));

            let has_not_parent_id_and_has_parent_with_key = StructInfo::Item(Arc::new(
                ItemBuilder::new()
                    .id((0, 0).into())
                    .left_id(None)
                    .right_id(None)
                    .parent(Some(Parent::String(String::from("parent"))))
                    .parent_sub(Some(String::from("parent_sub")))
                    .content(Content::String(String::from("content")))
                    .build(),
            ));

            let has_parent_id = StructInfo::Item(Arc::new(
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
                StructInfo::GC {
                    id: (0, 0).into(),
                    len: 42,
                },
                StructInfo::Skip {
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
                let decoded = StructInfo::read(&mut decoder, info.id()).unwrap();

                assert_eq!(info, decoded);
            }
        });
    }

    fn struct_info_round_trip(info: &mut StructInfo) -> JwstCodecResult {
        if let StructInfo::Item(item) = info {
            if !item.is_valid() {
                return Ok(());
            }

            if item.content.countable() {
                item.flags.set_countable();
            }
        }
        let mut encoder = RawEncoder::default();
        info.write(&mut encoder)?;

        let ret = encoder.into_inner();
        let mut decoder = RawDecoder::new(ret);

        let decoded = StructInfo::read(&mut decoder, info.id())?;

        assert_eq!(info, &decoded);

        Ok(())
    }

    #[cfg(not(loom))]
    proptest! {
        #[test]
        #[cfg_attr(miri, ignore)]
        fn test_random_struct_info(mut infos in vec(any::<StructInfo>(), 0..10)) {
            for info in &mut infos {
                struct_info_round_trip(info).unwrap();
            }
        }
    }
}
