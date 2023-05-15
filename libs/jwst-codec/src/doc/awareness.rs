use super::*;
use std::{cmp::max, collections::hash_map::Entry, sync::Arc};

pub struct Awareness {
    awareness: AwarenessStates,
    callback: Option<Arc<dyn Fn(&Awareness, AwarenessEvent) + Send + Sync + 'static>>,
    local_id: u64,
}

impl Awareness {
    pub fn new(local_id: u64) -> Self {
        Self {
            awareness: AwarenessStates::new(),
            callback: None,
            local_id,
        }
    }

    pub fn on_update(&mut self, f: impl Fn(&Awareness, AwarenessEvent) + Send + Sync + 'static) {
        self.callback = Some(Arc::new(f));
    }

    pub fn get_states(&self) -> &AwarenessStates {
        &self.awareness
    }

    pub fn get_local_state(&self) -> Option<String> {
        self.awareness
            .get(&self.local_id)
            .map(|state| state.content.clone())
    }

    fn mut_local_state(&mut self) -> &mut AwarenessState {
        self.awareness.entry(self.local_id).or_default()
    }

    pub fn set_local_state(&mut self, content: String) {
        self.mut_local_state().set_content(content);
        if let Some(cb) = self.callback.as_ref() {
            cb(
                self,
                AwarenessEventBuilder::new().update(self.local_id).build(),
            );
        }
    }

    pub fn clear_local_state(&mut self) {
        self.mut_local_state().delete();
        if let Some(cb) = self.callback.as_ref() {
            cb(
                self,
                AwarenessEventBuilder::new().remove(self.local_id).build(),
            );
        }
    }

    pub fn apply_update(&mut self, update: AwarenessStates) {
        let mut event = AwarenessEventBuilder::new();

        for (client_id, state) in update {
            match self.awareness.entry(client_id) {
                Entry::Occupied(mut entry) => {
                    let prev_state = entry.get_mut();
                    if client_id == self.local_id {
                        // ignore remote update about local client and
                        // add clock to overwrite remote data
                        prev_state.set_clock(max(prev_state.clock, state.clock) + 1);
                        event.update(client_id);
                        continue;
                    }

                    if prev_state.clock < state.clock {
                        if state.is_deleted() {
                            prev_state.delete();
                            event.remove(client_id);
                        } else {
                            *prev_state = state;
                            event.update(client_id);
                        }
                    }
                }
                Entry::Vacant(entry) => {
                    entry.insert(state);
                    event.add(client_id);
                }
            }
        }

        if let Some(cb) = self.callback.as_ref() {
            cb(self, event.build());
        }
    }
}

pub struct AwarenessEvent {
    added: Vec<u64>,
    updated: Vec<u64>,
    removed: Vec<u64>,
}

impl AwarenessEvent {
    pub fn get_updated(&self, states: &AwarenessStates) -> AwarenessStates {
        states
            .iter()
            .filter(|(id, _)| {
                self.added.contains(id) || self.updated.contains(id) || self.removed.contains(id)
            })
            .map(|(id, state)| (*id, state.clone()))
            .collect()
    }
}

struct AwarenessEventBuilder {
    added: Vec<u64>,
    updated: Vec<u64>,
    removed: Vec<u64>,
}

impl AwarenessEventBuilder {
    fn new() -> Self {
        Self {
            added: Vec::new(),
            updated: Vec::new(),
            removed: Vec::new(),
        }
    }

    fn add(&mut self, client_id: u64) -> &mut Self {
        self.added.push(client_id);
        self
    }

    fn update(&mut self, client_id: u64) -> &mut Self {
        self.updated.push(client_id);
        self
    }

    fn remove(&mut self, client_id: u64) -> &mut Self {
        self.removed.push(client_id);
        self
    }

    fn build(&mut self) -> AwarenessEvent {
        AwarenessEvent {
            added: self.added.clone(),
            updated: self.updated.clone(),
            removed: self.removed.clone(),
        }
    }
}
