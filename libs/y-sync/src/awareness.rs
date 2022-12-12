use std::cell::RefCell;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::rc::{Rc, Weak};
use std::time::Instant;
use thiserror::Error;
use yrs::block::ClientID;
use yrs::updates::decoder::{Decode, Decoder};
use yrs::updates::encoder::{Encode, Encoder};
use yrs::Doc;

const NULL_STR: &str = "null";

/// The Awareness class implements a simple shared state protocol that can be used for non-persistent
/// data like awareness information (cursor, username, status, ..). Each client can update its own
/// local state and listen to state changes of remote clients.
///
/// Each client is identified by a unique client id (something we borrow from `doc.clientID`).
/// A client can override its own state by propagating a message with an increasing timestamp
/// (`clock`). If such a message is received, it is applied if the known state of that client is
/// older than the new state (`clock < new_clock`). If a client thinks that a remote client is
/// offline, it may propagate a message with `{ clock, state: null, client }`. If such a message is
/// received, and the known clock of that client equals the received clock, it will clean the state.
///
/// Before a client disconnects, it should propagate a `null` state with an updated clock.
pub struct Awareness {
    doc: Doc,
    states: HashMap<ClientID, String>,
    meta: HashMap<ClientID, MetaClientState>,
    on_update: Option<EventHandler<Event>>,
}

unsafe impl Send for Awareness {}
unsafe impl Sync for Awareness {}

impl Awareness {
    /// Creates a new instance of [Awareness] struct, which operates over a given document.
    /// Awareness instance has full ownership of that document. If necessary it can be accessed
    /// using either [Awareness::doc] or [Awareness::doc_mut] methods.
    pub fn new(doc: Doc) -> Self {
        Awareness {
            doc,
            on_update: None,
            states: HashMap::new(),
            meta: HashMap::new(),
        }
    }

    /// Returns a channel receiver for an incoming awareness events. This channel can be cloned.
    pub fn on_update<F>(&mut self, f: F) -> Subscription<Event>
    where
        F: Fn(&Awareness, &Event) -> () + 'static,
    {
        let eh = self.on_update.get_or_insert_with(EventHandler::default);
        eh.subscribe(f)
    }

    /// Returns a read-only reference to an underlying [Doc].
    pub fn doc(&self) -> &Doc {
        &self.doc
    }

    /// Returns a read-write reference to an underlying [Doc].
    pub fn doc_mut(&mut self) -> &mut Doc {
        &mut self.doc
    }

    /// Returns a globally unique client ID of an underlying [Doc].
    pub fn client_id(&self) -> ClientID {
        self.doc.client_id
    }

    /// Returns a state map of all of the clients tracked by current [Awareness] instance. Those
    /// states are identified by their corresponding [ClientID]s. The associated state is
    /// represented and replicated to other clients as a JSON string.
    pub fn clients(&self) -> &HashMap<ClientID, String> {
        &self.states
    }

    /// Returns a JSON string state representation of a current [Awareness] instance.
    pub fn local_state(&self) -> Option<&str> {
        Some(self.states.get(&self.doc.client_id)?.as_str())
    }

    /// Sets a current [Awareness] instance state to a corresponding JSON string. This state will
    /// be replicated to other clients as part of the [AwarenessUpdate] and it will trigger an event
    /// to be emitted if current instance was created using [Awareness::with_observer] method.
    ///
    pub fn set_local_state<S: Into<String>>(&mut self, json: S) {
        let client_id = self.doc.client_id;
        self.update_meta(client_id);
        let new: String = json.into();
        match self.states.entry(client_id) {
            Entry::Occupied(mut e) => {
                e.insert(new);
                if let Some(eh) = self.on_update.as_ref() {
                    eh.trigger(self, &Event::new(vec![], vec![client_id], vec![]));
                }
            }
            Entry::Vacant(e) => {
                e.insert(new);
                if let Some(eh) = self.on_update.as_ref() {
                    eh.trigger(self, &Event::new(vec![client_id], vec![], vec![]));
                }
            }
        }
    }

    /// Clears out a state of a given client, effectively marking it as disconnected.
    pub fn remove_state(&mut self, client_id: ClientID) {
        let prev_state = self.states.remove(&client_id);
        self.update_meta(client_id);
        if let Some(eh) = self.on_update.as_ref() {
            if prev_state.is_some() {
                eh.trigger(
                    self,
                    &Event::new(Vec::default(), Vec::default(), vec![client_id]),
                );
            }
        }
    }

    /// Clears out a state of a current client (see: [Awareness::client_id]),
    /// effectively marking it as disconnected.
    pub fn clean_local_state(&mut self) {
        let client_id = self.doc.client_id;
        self.remove_state(client_id);
    }

    fn update_meta(&mut self, client_id: ClientID) {
        match self.meta.entry(client_id) {
            Entry::Occupied(mut e) => {
                let clock = e.get().clock + 1;
                let meta = MetaClientState::new(clock, Instant::now());
                e.insert(meta);
            }
            Entry::Vacant(e) => {
                e.insert(MetaClientState::new(1, Instant::now()));
            }
        }
    }

    /// Returns a serializable update object which is representation of a current Awareness state.
    pub fn update(&self) -> Result<AwarenessUpdate, Error> {
        let clients = self.states.keys().cloned();
        self.update_with_clients(clients)
    }

    /// Returns a serializable update object which is representation of a current Awareness state.
    /// Unlike [Awareness::update], this method variant allows to prepare update only for a subset
    /// of known clients. These clients must all be known to a current [Awareness] instance,
    /// otherwise a [Error::ClientNotFound] error will be returned.
    pub fn update_with_clients<I: IntoIterator<Item = ClientID>>(
        &self,
        clients: I,
    ) -> Result<AwarenessUpdate, Error> {
        let mut res = HashMap::new();
        for client_id in clients {
            let clock = if let Some(meta) = self.meta.get(&client_id) {
                meta.clock
            } else {
                return Err(Error::ClientNotFound(client_id));
            };
            let json = if let Some(json) = self.states.get(&client_id) {
                json.clone()
            } else {
                String::from(NULL_STR)
            };
            res.insert(client_id, AwarenessUpdateEntry { clock, json });
        }
        Ok(AwarenessUpdate { clients: res })
    }

    /// Applies an update (incoming from remote channel or generated using [Awareness::update] /
    /// [Awareness::update_with_clients] methods) and modifies a state of a current instance.
    ///
    /// If current instance has an observer channel (see: [Awareness::with_observer]), applied
    /// changes will also be emitted as events.
    pub fn apply_update(&mut self, update: AwarenessUpdate) -> Result<(), Error> {
        let now = Instant::now();

        let mut added = Vec::new();
        let mut updated = Vec::new();
        let mut removed = Vec::new();

        for (client_id, entry) in update.clients {
            let mut clock = entry.clock;
            let is_null = entry.json.as_str() == NULL_STR;
            match self.meta.entry(client_id) {
                Entry::Occupied(mut e) => {
                    let prev = e.get();
                    let is_removed =
                        prev.clock == clock && is_null && self.states.contains_key(&client_id);
                    let is_new = prev.clock < clock;
                    if is_new || is_removed {
                        if is_null {
                            // never let a remote client remove this local state
                            if client_id == self.doc.client_id
                                && self.states.get(&client_id).is_some()
                            {
                                // remote client removed the local state. Do not remote state. Broadcast a message indicating
                                // that this client still exists by increasing the clock
                                clock += 1;
                            } else {
                                self.states.remove(&client_id);
                                if self.on_update.is_some() {
                                    removed.push(client_id);
                                }
                            }
                        } else {
                            match self.states.entry(client_id) {
                                Entry::Occupied(mut e) => {
                                    if self.on_update.is_some() {
                                        updated.push(client_id);
                                    }
                                    e.insert(entry.json);
                                }
                                Entry::Vacant(e) => {
                                    e.insert(entry.json);
                                    if self.on_update.is_some() {
                                        updated.push(client_id);
                                    }
                                }
                            }
                        }
                        e.insert(MetaClientState::new(clock, now));
                        true
                    } else {
                        false
                    }
                }
                Entry::Vacant(e) => {
                    e.insert(MetaClientState::new(clock, now));
                    self.states.insert(client_id, entry.json);
                    if self.on_update.is_some() {
                        added.push(client_id);
                    }
                    true
                }
            };
        }

        if let Some(eh) = self.on_update.as_ref() {
            if !added.is_empty() || !updated.is_empty() || !removed.is_empty() {
                eh.trigger(self, &Event::new(added, updated, removed));
            }
        }

        Ok(())
    }
}

impl Default for Awareness {
    fn default() -> Self {
        Awareness::new(Doc::new())
    }
}

struct EventHandler<T> {
    seq_nr: u32,
    subscribers: Rc<RefCell<HashMap<u32, Box<dyn Fn(&Awareness, &T) -> ()>>>>,
}

impl<T> EventHandler<T> {
    pub fn subscribe<F>(&mut self, f: F) -> Subscription<T>
    where
        F: Fn(&Awareness, &T) -> () + 'static,
    {
        let subscription_id = self.seq_nr;
        self.seq_nr += 1;
        {
            let func = Box::new(f);
            let mut subs = self.subscribers.borrow_mut();
            subs.insert(subscription_id, func);
        }
        Subscription {
            subscription_id,
            subscribers: Rc::downgrade(&self.subscribers),
        }
    }

    pub fn trigger(&self, awareness: &Awareness, arg: &T) {
        let subs = self.subscribers.borrow();
        for func in subs.values() {
            func(awareness, arg);
        }
    }
}

impl<T> Default for EventHandler<T> {
    fn default() -> Self {
        EventHandler {
            seq_nr: 0,
            subscribers: Rc::new(RefCell::new(HashMap::new())),
        }
    }
}

/// Whenever a new callback is being registered, a [Subscription] is made. Whenever this
/// subscription a registered callback is cancelled and will not be called any more.
pub struct Subscription<T> {
    subscription_id: u32,
    subscribers: Weak<RefCell<HashMap<u32, Box<dyn Fn(&Awareness, &T) -> ()>>>>,
}

impl<T> Drop for Subscription<T> {
    fn drop(&mut self) {
        if let Some(subs) = self.subscribers.upgrade() {
            let mut s = subs.borrow_mut();
            s.remove(&self.subscription_id);
        }
    }
}

/// A structure that represents an encodable state of an [Awareness] struct.
#[derive(Debug, Eq, PartialEq)]
pub struct AwarenessUpdate {
    clients: HashMap<ClientID, AwarenessUpdateEntry>,
}

impl Encode for AwarenessUpdate {
    fn encode<E: Encoder>(&self, encoder: &mut E) {
        encoder.write_var(self.clients.len());
        for (&client_id, e) in self.clients.iter() {
            encoder.write_var(client_id);
            encoder.write_var(e.clock);
            encoder.write_string(&e.json);
        }
    }
}

impl Decode for AwarenessUpdate {
    fn decode<D: Decoder>(decoder: &mut D) -> Result<Self, lib0::error::Error> {
        let len: usize = decoder.read_var()?;
        let mut clients = HashMap::with_capacity(len);
        for _ in 0..len {
            let client_id: ClientID = decoder.read_var()?;
            let clock: u32 = decoder.read_var()?;
            let json = decoder.read_string()?.to_string();
            clients.insert(client_id, AwarenessUpdateEntry { clock, json });
        }

        Ok(AwarenessUpdate { clients })
    }
}

/// A single client entry of an [AwarenessUpdate]. It consists of logical clock and JSON client
/// state represented as a string.
#[derive(Debug, Eq, PartialEq)]
pub struct AwarenessUpdateEntry {
    clock: u32,
    json: String,
}

/// Errors generated by an [Awareness] struct methods.
#[derive(Error, Debug)]
pub enum Error {
    /// Client ID was not found in [Awareness] metadata.
    #[error("client ID `{0}` not found")]
    ClientNotFound(ClientID),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MetaClientState {
    clock: u32,
    last_updated: Instant,
}

impl MetaClientState {
    fn new(clock: u32, last_updated: Instant) -> Self {
        MetaClientState {
            clock,
            last_updated,
        }
    }
}

/// Event type emitted by an [Awareness] struct.
#[derive(Debug, Default, Clone, Eq, PartialEq)]
pub struct Event {
    added: Vec<ClientID>,
    updated: Vec<ClientID>,
    removed: Vec<ClientID>,
}

impl Event {
    pub fn new(added: Vec<ClientID>, updated: Vec<ClientID>, removed: Vec<ClientID>) -> Self {
        Event {
            added,
            updated,
            removed,
        }
    }

    /// Collection of new clients that have been added to an [Awareness] struct, that was not known
    /// before. Actual client state can be accessed via `awareness.clients().get(client_id)`.
    pub fn added(&self) -> &[ClientID] {
        &self.added
    }

    /// Collection of new clients that have been updated within an [Awareness] struct since the last
    /// update. Actual client state can be accessed via `awareness.clients().get(client_id)`.
    pub fn updated(&self) -> &[ClientID] {
        &self.updated
    }

    /// Collection of new clients that have been removed from [Awareness] struct since the last
    /// update.
    pub fn removed(&self) -> &[ClientID] {
        &self.removed
    }
}

#[cfg(test)]
mod test {
    use crate::awareness::{Awareness, Event};
    use std::sync::mpsc::{channel, Receiver};
    use yrs::Doc;

    fn update(
        recv: &mut Receiver<Event>,
        from: &Awareness,
        to: &mut Awareness,
    ) -> Result<Event, Box<dyn std::error::Error>> {
        let e = recv.try_recv()?;
        let u = from.update_with_clients([e.added(), e.updated(), e.removed()].concat())?;
        to.apply_update(u)?;
        Ok(e)
    }

    #[test]
    fn awareness() -> Result<(), Box<dyn std::error::Error>> {
        let (s1, mut o_local) = channel();
        let mut local = Awareness::new(Doc::with_client_id(1));
        let _sub_local = local.on_update(move |_, e| {
            s1.send(e.clone()).unwrap();
        });

        let (s2, o_remote) = channel();
        let mut remote = Awareness::new(Doc::with_client_id(2));
        let _sub_remote = local.on_update(move |_, e| {
            s2.send(e.clone()).unwrap();
        });

        local.set_local_state("{x:3}");
        let mut e_local = update(&mut o_local, &local, &mut remote)?;
        assert_eq!(remote.clients()[&1], "{x:3}");
        assert_eq!(remote.meta[&1].clock, 1);
        assert_eq!(o_remote.try_recv()?.added, &[1]);

        local.set_local_state("{x:4}");
        e_local = update(&mut o_local, &local, &mut remote)?;
        let e_remote = o_remote.try_recv()?;
        assert_eq!(remote.clients()[&1], "{x:4}");
        assert_eq!(e_remote, Event::new(vec![], vec![1], vec![]));
        assert_eq!(e_remote, e_local);

        local.clean_local_state();
        e_local = update(&mut o_local, &local, &mut remote)?;
        let e_remote = o_remote.try_recv()?;
        assert_eq!(e_remote.removed.len(), 1);
        assert_eq!(local.clients().get(&1), None);
        assert_eq!(e_remote, e_local);
        Ok(())
    }
}
