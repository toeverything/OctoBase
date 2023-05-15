use super::*;
use nom::multi::count;
use std::collections::{HashMap, VecDeque};

#[derive(Debug)]
pub struct Delete {
    pub clock: Clock,
    pub len: u64,
}

#[derive(Debug)]
pub struct DeleteSets {
    pub client: u64,
    pub deletes: Vec<Delete>,
}

#[derive(Debug, Default)]
pub struct Update {
    pub delete_sets: Vec<DeleteSets>,
    pub structs: HashMap<u64, VecDeque<StructInfo>>,

    /// all unapplicable items that we can't integrate into doc
    /// any item with inconsistent id clock or missing dependency will be put here
    rest_structs: HashMap<Client, Vec<StructInfo>>,
    /// missing state vector after applying updates
    missing_state: StateVector,
}

fn read_delete(input: &[u8]) -> IResult<&[u8], Delete> {
    let (tail, clock) = read_var_u64(input)?;
    let (tail, len) = read_var_u64(tail)?;
    Ok((tail, Delete { clock, len }))
}

fn parse_delete_set(input: &[u8]) -> IResult<&[u8], DeleteSets> {
    let (input, client) = read_var_u64(input)?;
    let (input, num_of_deletes) = read_var_u64(input)?;
    let (tail, deletes) = count(read_delete, num_of_deletes as usize)(input)?;

    Ok((tail, DeleteSets { client, deletes }))
}

fn read_delete_set(input: &[u8]) -> IResult<&[u8], Vec<DeleteSets>> {
    let (input, num_of_clients) = read_var_u64(input)?;
    let (tail, deletes) = count(parse_delete_set, num_of_clients as usize)(input)?;

    Ok((tail, deletes))
}

pub fn read_update(input: &[u8]) -> IResult<&[u8], Update> {
    let (tail, structs) = read_client_struct_refs(input)?;
    let (tail, delete_sets) = read_delete_set(tail)?;
    Ok((
        tail,
        Update {
            structs,
            delete_sets,
            ..Update::default()
        },
    ))
}

pub struct UpdateIterator<'a> {
    update: &'a mut Update,

    // --- local iterator state ---
    /// current state vector from store
    state: StateVector,
    /// all client ids sorted ascending
    client_ids: Vec<Client>,
    /// current id of client of the updates we're processing
    cur_client_id: Option<Client>,
    /// stack of previous iterating item with higher priority than updates in next iteration
    stack: Vec<StructInfo>,
}

impl<'a> UpdateIterator<'a> {
    pub fn new(store: &DocStore, update: &'a mut Update) -> Self {
        let state = store.get_state_vector();
        let mut client_ids = update.structs.keys().cloned().collect::<Vec<_>>();
        client_ids.sort();
        let cur_client_id = client_ids.pop();

        UpdateIterator {
            update,
            state,
            client_ids,
            cur_client_id,
            stack: Vec::new(),
        }
    }

    /// iterate the client ids until we find the next client with left updates that can be consumed
    ///
    /// note:
    /// firstly we will check current client id as well to ensure current updates queue is not empty yet
    ///
    fn next_client(&mut self) -> Option<Client> {
        while let Some(client_id) = self.cur_client_id {
            match self.update.structs.get(&client_id) {
                Some(refs) if !refs.is_empty() => {
                    self.cur_client_id.replace(client_id);
                    return self.cur_client_id;
                }
                _ => {
                    self.cur_client_id = self.client_ids.pop();
                }
            }
        }

        None
    }

    /// update the missing state vector
    /// tell it the smallest clock that missed.
    fn update_missing_state(&mut self, client: Client, clock: Clock) {
        self.update.missing_state.set_min(client, clock);
    }

    /// any time we can't apply an update during the iteration,
    /// we should put all items in pending stack to rest structs
    fn add_stack_to_rest(&mut self) {
        for s in self.stack.drain(..) {
            let client = s.id().client;
            let unapplicable_items = self.update.structs.remove(&client);
            if let Some(mut items) = unapplicable_items {
                items.push_front(s);
                self.update.rest_structs.insert(client, items.into());
            } else {
                self.update.rest_structs.insert(client, vec![s.clone()]);
            }
            self.client_ids.retain(|&c| c != client);
        }
    }

    /// tell if current update's dependencies(left, right, parent) has already been consumed and recorded
    /// and return the client of them if not.
    fn get_missing_dep(&self, struct_info: &StructInfo) -> Option<Client> {
        if let StructInfo::Item { id, item } = struct_info {
            if let Some(left) = &item.left_id {
                if left.client != id.client && left.clock >= self.state.get(&left.client) {
                    return Some(left.client);
                }
            }

            if let Some(right) = &item.right_id {
                if right.client != id.client && right.clock >= self.state.get(&right.client) {
                    return Some(right.client);
                }
            }

            if let Some(parent) = &item.parent {
                match parent {
                    Parent::Id(parent_id)
                        if parent_id.client != id.client
                            && parent_id.clock >= self.state.get(&parent_id.client) =>
                    {
                        return Some(parent_id.client);
                    }
                    _ => {}
                }
            }
        }

        None
    }

    fn next_candidate(&mut self) -> Option<StructInfo> {
        let mut cur = None;

        if !self.stack.is_empty() {
            cur.replace(self.stack.pop().unwrap());
        } else if let Some(client) = self.next_client() {
            // Safety:
            // client index of updates and update length are both checked in next_client
            // safe to use unwrap
            cur.replace(
                self.update
                    .structs
                    .get_mut(&client)
                    .unwrap()
                    .pop_front()
                    .unwrap(),
            );
        }

        cur
    }
}

impl Update {
    pub fn iter(&mut self, store: &DocStore) -> UpdateIterator {
        UpdateIterator::new(store, self)
    }
}

impl<'a> Iterator for UpdateIterator<'a> {
    type Item = (StructInfo, u64);

    fn next(&mut self) -> Option<Self::Item> {
        // fetch the first candidate from stack or updates
        let mut cur = self.next_candidate();

        while let Some(cur_update) = cur.take() {
            let id = *cur_update.id();
            if cur_update.is_skip() {
                cur = self.next_candidate();
                continue;
            } else if !self.state.contains(&id) {
                // missing local state of same client
                // can't apply the continuous updates from same client
                // push into the stack and put tell all the items in stack are unapplicable
                self.stack.push(cur_update);
                self.update_missing_state(id.client, id.clock - 1);
                self.add_stack_to_rest();
            } else {
                let id = cur_update.id();
                let dep = self.get_missing_dep(&cur_update);
                // some dependency is missing, we need to turn to iterate the dependency first.
                if let Some(dep) = dep {
                    self.stack.push(cur_update);

                    match self.update.structs.get_mut(&dep) {
                        Some(updates) if !updates.is_empty() => {
                            // iterate the dependency client first
                            cur.replace(updates.pop_front().unwrap());
                            continue;
                        }
                        // but the dependency update is drained
                        // need to move all stack item to unapplicable store
                        _ => {
                            self.update_missing_state(dep, self.state.get(&dep));
                            self.add_stack_to_rest();
                        }
                    }
                } else {
                    // we finally find the first applicable update
                    let local_state = self.state.get(&id.client);
                    // we've already check the local state is greater or equal to current update's clock
                    // so offset here will never be negative
                    let offset = local_state - id.clock;
                    if offset == 0 || offset < cur_update.len() {
                        self.state.set_max(id.client, id.clock + cur_update.len());
                        return Some((cur_update, offset));
                    }
                }
            }

            cur = self.next_candidate();
        }

        // we all done
        None
    }
}
