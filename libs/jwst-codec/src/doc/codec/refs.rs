use super::*;

#[derive(Debug, Clone)]
#[cfg_attr(test, derive(proptest_derive::Arbitrary))]
pub enum StructInfo {
    GC { id: Id, len: u64 },
    Skip { id: Id, len: u64 },
    Item { id: Id, item: Box<Item> },
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
            StructInfo::Item { item, .. } => item.write(writer),
        }
    }
}

impl PartialEq for StructInfo {
    fn eq(&self, other: &Self) -> bool {
        self.id() == other.id()
    }
}

impl Eq for StructInfo {
    fn assert_receiver_is_total_eq(&self) {}
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
            _ => Ok(StructInfo::Item {
                id,
                item: Box::new(Item::read(decoder, info, first_5_bit)?),
            }),
        }
    }

    pub fn id(&self) -> Id {
        *match self {
            StructInfo::GC { id, .. } => id,
            StructInfo::Skip { id, .. } => id,
            StructInfo::Item { id, .. } => id,
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
            Self::Item { item, .. } => item.content.clock_len(),
        }
    }

    pub fn is_gc(&self) -> bool {
        matches!(self, Self::GC { .. })
    }

    pub fn is_skip(&self) -> bool {
        matches!(self, Self::Skip { .. })
    }

    pub fn is_item(&self) -> bool {
        matches!(self, Self::Item { .. })
    }

    pub fn flags(&self) -> ItemFlags {
        if let StructInfo::Item { item, .. } = self {
            item.flags
        } else {
            ItemFlags::from(0)
        }
    }

    pub fn item(&self) -> Option<&Item> {
        if let Self::Item { item, .. } = self {
            Some(item.as_ref())
        } else {
            None
        }
    }

    pub fn left_id(&self) -> Option<Id> {
        if let Self::Item { item, .. } = self {
            item.left_id
        } else {
            None
        }
    }

    pub fn right_id(&self) -> Option<Id> {
        if let Self::Item { item, .. } = self {
            item.right_id
        } else {
            None
        }
    }

    pub fn parent(&self) -> Option<&Parent> {
        if let Self::Item { item, .. } = self {
            item.parent.as_ref()
        } else {
            None
        }
    }

    pub fn parent_sub(&self) -> Option<&String> {
        if let Self::Item { item, .. } = self {
            item.parent_sub.as_ref()
        } else {
            None
        }
    }

    pub fn split_item(&mut self, diff: u64) -> JwstCodecResult<Self> {
        if let Self::Item { id, item } = self {
            let right_id = Id::new(id.client, id.clock + diff);
            item.right_id = Some(right_id);
            let right_content = item.content.split(diff)?;

            let right_item = Self::Item {
                id: right_id,
                item: Box::new(Item {
                    left_id: Some(Id::new(id.client, id.clock + diff - 1)),
                    right_id: item.right_id,
                    content: right_content,
                    ..item.as_ref().clone()
                }),
            };

            Ok(right_item)
        } else {
            Err(JwstCodecError::ItemSplitNotSupport)
        }
    }

    pub fn delete(&mut self) {
        if let StructInfo::Item { item, .. } = self {
            item.delete()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::{collection::vec, prelude::*};
    use super::item_util::ItemBuilder;

    #[test]
    fn test_struct_info() {
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
                .left_id(None)
                .right_id(None)
                .parent(Some(Parent::String(String::from("parent"))))
                .parent_sub(None)
                .content(Content::String(String::from("content")))
                .build();
            let struct_info = StructInfo::Item {
                id: Id::new(3, 0),
                item: Box::new(item),
            };

            assert_eq!(struct_info.len(), 7);
            assert_eq!(struct_info.client(), 3);
            assert_eq!(struct_info.clock(), 0);
        }
    }

    #[test]
    fn test_read_write_struct_info() {
        let has_not_parent_id_and_has_parent = StructInfo::Item {
            id: (0, 0).into(),
            item: Box::new(
                ItemBuilder::new()
                    .left_id(None)
                    .right_id(None)
                    .parent(Some(Parent::String(String::from("parent"))))
                    .parent_sub(None)
                    .content(Content::String(String::from("content")))
                    .build(),
            ),
        };

        let has_not_parent_id_and_has_parent_with_key = StructInfo::Item {
            id: (0, 0).into(),
            item: Box::new(
                ItemBuilder::new()
                    .left_id(None)
                    .right_id(None)
                    .parent(Some(Parent::String(String::from("parent"))))
                    .parent_sub(Some(String::from("parent_sub")))
                    .content(Content::String(String::from("content")))
                    .build(),
            ),
        };

        let has_parent_id = StructInfo::Item {
            id: (0, 0).into(),
            item: Box::new(
                ItemBuilder::new()
                    .left_id(Some((1, 2).into()))
                    .right_id(Some((2, 5).into()))
                    .parent(None)
                    .parent_sub(None)
                    .content(Content::String(String::from("content")))
                    .build(),
            ),
        };

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
    }

    fn struct_info_round_trip(info: &mut StructInfo) -> JwstCodecResult {
        if let StructInfo::Item { item, .. } = info {
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

    proptest! {
        #[test]
        fn test_random_struct_info(mut infos in vec(any::<StructInfo>(), 0..10)) {
            for info in &mut infos {
                struct_info_round_trip(info).unwrap();
            }
        }
    }
}
