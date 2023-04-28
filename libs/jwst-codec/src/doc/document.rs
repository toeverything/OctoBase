use std::{
    collections::{HashMap, HashSet},
    ops::{Deref, DerefMut},
    sync::Arc,
};

use super::*;

#[derive(Default, Debug)]
pub struct StateVector(HashMap<Client, Clock>);

impl StateVector {
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    pub fn set_max(&mut self, client: Client, clock: Clock) {
        self.entry(client)
            .and_modify(|mclock| {
                if *mclock < clock {
                    *mclock = clock;
                }
            })
            .or_insert(clock);
    }

    pub fn get(&self, client: &Client) -> Clock {
        *self.0.get(client).unwrap_or(&0)
    }

    pub fn contains(&self, id: &Id) -> bool {
        id.clock <= self.get(&id.client)
    }

    pub fn set_min(&mut self, client: Client, clock: Clock) {
        self.entry(client)
            .and_modify(|mclock| {
                if *mclock > clock {
                    *mclock = clock;
                }
            })
            .or_insert(clock);
    }
}

impl Deref for StateVector {
    type Target = HashMap<Client, Clock>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for StateVector {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl From<&HashMap<Client, Vec<StructInfo>>> for StateVector {
    fn from(value: &HashMap<Client, Vec<StructInfo>>) -> Self {
        Self(
            value
                .iter()
                .map(|(k, v)| {
                    (
                        *k,
                        match v.last() {
                            Some(v) => v.id().clock + v.len(),
                            _ => 0,
                        },
                    )
                })
                .collect(),
        )
    }
}

pub struct Doc {
    // TODO: use function in code
    #[allow(dead_code)]
    // random client id for each doc
    client_id: u64,
    // TODO: use function in code
    #[allow(dead_code)]
    // random id for each doc, use in sub doc
    guid: String,
    // root_type: HashMap<String, Item>,
    store: DocStore,
}

impl Default for Doc {
    fn default() -> Self {
        Self {
            client_id: rand::random(),
            guid: nanoid!(),
            // share: HashMap::new(),
            store: DocStore::new(),
        }
    }
}

impl Doc {
    pub fn apply_update(&mut self, update: &[u8]) -> JwstCodecResult {
        let (rest, mut update) = read_update(update).map_err(|e| e.map_input(|u| u.len()))?;
        if !rest.is_empty() {
            return Err(JwstCodecError::UpdateNotFullyConsumed(rest.len()));
        }

        self.integrate_update(&mut update)?;

        // TODO: deal with the pending updates that can't be applied
        // update.missing_state
        // update.rest_updates

        Ok(())
    }

    fn integrate_update(&mut self, update: &mut Update) -> JwstCodecResult {
        for (s, offset) in update.iter(&self.store) {
            self.integrate_struct_info(s, offset)?;
        }

        Ok(())
    }

    fn integrate_struct_info(
        &mut self,
        mut struct_info: StructInfo,
        offset: u64,
    ) -> JwstCodecResult<()> {
        self.repair(&mut struct_info)?;
        match &mut struct_info {
            StructInfo::Item { id, item } => {
                let mut left =
                    item.left_id
                        .and_then(|left_id| match self.store.get_item(left_id) {
                            Ok(left) => Some(left),
                            _ => None,
                        });

                let right =
                    item.right_id
                        .and_then(|right_id| match self.store.get_item(right_id) {
                            Ok(right) => Some(right),
                            _ => None,
                        });

                let parent = item.parent.as_ref().and_then(|parent| match parent {
                    Parent::String(_) => todo!(),
                    Parent::Id(id) => Some(*id),
                });

                if offset > 0 {
                    id.clock += offset;
                    left = Some(
                        self.store
                            .get_item_clean_end(Id::new(id.client, id.clock - 1))?,
                    );
                    item.content.split(offset)?;
                }

                if let Some(_parent_id) = parent {
                    let right_is_null_or_has_left = match &right {
                        None => true,
                        Some(right) => right.left_id().is_some(),
                    };
                    let left_has_other_right_than_self = match &left {
                        Some(left) => left.right_id().is_some(),
                        _ => false,
                    };

                    // conflicts
                    if (left.is_none() && right_is_null_or_has_left)
                        || left_has_other_right_than_self
                    {
                        // set the first conflicting item
                        let mut o = if let Some(left) = left.clone() {
                            left.right_id()
                        } else if let Some(_sub) = &item.parent_sub {
                            // TODO: handle parent_sub, set if parent is Map or Array
                            unimplemented!()
                        } else {
                            // TODO: handle parent w/o sub, occurs if parent is Text
                            unimplemented!()
                        };

                        let mut conflicting_items = HashSet::new();
                        let mut items_before_origin = HashSet::new();

                        while let Some(conflict_id) = o {
                            match right {
                                Some(right) if right.id() == &conflict_id => {
                                    break;
                                }
                                _ => {}
                            };

                            items_before_origin.insert(conflict_id);
                            conflicting_items.insert(conflict_id);
                            let conflict_item = self.store.get_item(conflict_id)?;
                            match conflict_item.as_ref() {
                                StructInfo::Item { item: c, .. } => {
                                    if item.left_id == c.left_id {
                                        // case 1
                                        if conflict_id.client < id.client {
                                            left = Some(Arc::clone(&conflict_item));
                                            conflicting_items.clear();
                                        } else if item.right_id == c.right_id {
                                            // `this` and `c` are conflicting and point to the same
                                            // integration points. The id decides which item comes first.
                                            // Since `this` is to the left of `c`, we can break here.
                                            break;
                                        }
                                    } else if let Some(conflict_item_left) = c.left_id {
                                        if items_before_origin.contains(&conflict_item_left)
                                            && !conflicting_items.contains(&conflict_item_left)
                                        {
                                            left = Some(Arc::clone(&conflict_item));
                                            conflicting_items.clear();
                                        }
                                    } else {
                                        break;
                                    }
                                    o = conflict_item.right_id();
                                }
                                _ => {
                                    break;
                                }
                            }
                        }
                        item.left_id = left.map(|item| *item.id())
                    }
                }
            }
            StructInfo::GC { id, len } => {
                if offset > 0 {
                    id.clock += offset;
                    *len -= offset;
                }
            }
            StructInfo::Skip { .. } => {
                // skip ignored
            }
        }
        self.store.add_item(struct_info)
    }

    fn repair(&mut self, info: &mut StructInfo) -> JwstCodecResult {
        if let StructInfo::Item { item, .. } = info {
            if let Some(left_id) = item.left_id {
                let left = self.store.get_item_clean_end(left_id)?;
                item.left_id = Some(*left.id());
            }

            if let Some(right_id) = item.right_id {
                let right = self.store.get_item_clean_start(right_id)?;
                item.right_id = Some(*right.id());
            }
        }

        Ok(())
    }
}
