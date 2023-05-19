use super::*;
use std::collections::VecDeque;

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
    fn write(&self, encoder: &mut W) -> JwstCodecResult<()> {
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

#[derive(Debug, Clone, PartialEq)]
pub enum StructInfo {
    GC { id: Id, len: u64 },
    Skip { id: Id, len: u64 },
    Item { id: Id, item: Box<Item> },
}

impl StructInfo {
    pub fn id(&self) -> &Id {
        match self {
            Self::GC { id, .. } => id,
            Self::Skip { id, .. } => id,
            Self::Item { id, .. } => id,
        }
    }

    pub fn client_id(&self) -> u64 {
        self.id().client
    }

    pub fn clock(&self) -> u64 {
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

    pub fn split_item(&self, diff: u64) -> JwstCodecResult<(Self, Self)> {
        if let Self::Item { id, item } = self {
            let right_id = Id::new(id.client, id.clock + diff);
            let (left_content, right_content) = item.content.split(diff)?;

            let left_item = Self::Item {
                id: *id,
                item: Box::new(Item {
                    right_id: Some(right_id),
                    content: left_content,
                    ..item.as_ref().clone()
                }),
            };

            let right_item = Self::Item {
                id: right_id,
                item: Box::new(Item {
                    left_id: Some(Id::new(id.client, id.clock + diff - 1)),
                    right_id: item.right_id,
                    parent: item.parent.clone(),
                    parent_sub: item.parent_sub.clone(),
                    content: right_content,
                }),
            };

            Ok((left_item, right_item))
        } else {
            Err(JwstCodecError::ItemSplitNotSupport)
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

    #[test]
    fn test_struct_info() {
        {
            let struct_info = StructInfo::GC {
                id: Id::new(1, 0),
                len: 10,
            };
            assert_eq!(struct_info.len(), 10);
            assert_eq!(struct_info.client_id(), 1);
            assert_eq!(struct_info.clock(), 0);
        }

        {
            let struct_info = StructInfo::Skip {
                id: Id::new(2, 0),
                len: 20,
            };
            assert_eq!(struct_info.len(), 20);
            assert_eq!(struct_info.client_id(), 2);
            assert_eq!(struct_info.clock(), 0);
        }
    }
}
