use jwst_codec::{Awareness, AwarenessEvent, History};

use super::*;

impl Workspace {
    pub async fn subscribe_awareness(&self, f: impl Fn(&Awareness, AwarenessEvent) + Send + Sync + 'static) {
        self.awareness.write().unwrap().on_update(f);
    }

    pub fn subscribe_count(&self) -> usize {
        self.doc.subscribe_count()
    }

    pub fn subscribe_doc(&self, f: impl Fn(&[u8], &[History]) + Sync + Send + 'static) {
        self.doc.publisher.start();
        self.doc.subscribe(f)
    }

    pub fn unsubscribe_all(&self) {
        self.doc.unsubscribe_all();
        self.doc.publisher.stop();
        self.awareness.write().unwrap().on_update(|_, _| {})
    }
}
