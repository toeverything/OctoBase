use super::*;
use std::collections::VecDeque;

#[derive(Debug, PartialEq)]
#[cfg_attr(test, derive(proptest_derive::Arbitrary))]
enum RawStructInfo {
    GC(u64),
    Skip(u64),
    Item(Item),
}

impl<R: CrdtReader> CrdtRead<R> for RawStructInfo {
    fn read(decoder: &mut R) -> JwstCodecResult<Self> {
        let info = decoder.read_info()?;
        let first_5_bit = info & 0b11111;

        match first_5_bit {
            0 => {
                let len = decoder.read_var_u64()?;
                Ok(Self::GC(len))
            }
            10 => {
                let len = decoder.read_var_u64()?;
                Ok(Self::Skip(len))
            }
            _ => Ok(Self::Item(Item::read(decoder, info, first_5_bit)?)),
        }
    }
}

impl<W: CrdtWriter> CrdtWrite<W> for RawStructInfo {
    fn write(&self, encoder: &mut W) -> JwstCodecResult {
        match self {
            Self::GC(len) => {
                encoder.write_info(0)?;
                encoder.write_var_u64(*len)?;
            }
            Self::Skip(len) => {
                encoder.write_info(10)?;
                encoder.write_var_u64(*len)?;
            }
            Self::Item(item) => {
                item.write(encoder)?;
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub enum StructInfo {
    GC { id: Id, len: u64 },
    Skip { id: Id, len: u64 },
    Item { id: Id, item: Box<Item> },
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

pub struct RawRefs {
    pub(crate) client: u64,
    pub(crate) refs: VecDeque<StructInfo>,
}

impl<R: CrdtReader> CrdtRead<R> for RawRefs {
    fn read(decoder: &mut R) -> JwstCodecResult<Self> {
        let num_of_structs = decoder.read_var_u64()?;
        let client = decoder.read_var_u64()?;
        let clock = decoder.read_var_u64()?;
        let structs = (0..num_of_structs)
            .map(|_| RawStructInfo::read(decoder))
            .collect::<Result<Vec<_>, _>>()?;

        let (refs, _) = structs.into_iter().fold(
            (VecDeque::with_capacity(num_of_structs as usize), clock),
            |(mut vec, clock), s| {
                let id = Id::new(client, clock);
                match s {
                    RawStructInfo::GC(len) => {
                        vec.push_back(StructInfo::GC { id, len });
                        (vec, clock + len)
                    }
                    RawStructInfo::Skip(len) => {
                        vec.push_back(StructInfo::Skip { id, len });
                        (vec, clock + len)
                    }
                    RawStructInfo::Item(item) => {
                        let len = item.content.clock_len();
                        vec.push_back(StructInfo::Item {
                            id,
                            item: Box::new(item),
                        });
                        (vec, clock + len)
                    }
                }
            },
        );

        Ok(Self { client, refs })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::{collection::vec, prelude::*};

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
    }

    #[test]
    fn test_raw_struct_info() {
        let raw_struct_infos = vec![
            RawStructInfo::GC(42),
            RawStructInfo::Skip(314),
            // RawStructInfo::Item(Item::new()),
        ];

        for info in raw_struct_infos {
            let mut encoder = RawEncoder::default();
            info.write(&mut encoder).unwrap();

            let mut decoder = RawDecoder::new(encoder.into_inner());
            let decoded = RawStructInfo::read(&mut decoder).unwrap();

            assert_eq!(info, decoded);
        }
    }

    fn struct_info_round_trip(info: &mut RawStructInfo) -> JwstCodecResult {
        if let RawStructInfo::Item(item) = info {
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

        let decoded = RawStructInfo::read(&mut decoder)?;

        assert_eq!(info, &decoded);

        Ok(())
    }

    proptest! {
        #[test]
        fn test_random_struct_info(mut infos in vec(any::<RawStructInfo>(), 0..10)) {
            for info in &mut infos {
                struct_info_round_trip(info).unwrap();
            }
        }
    }
}
