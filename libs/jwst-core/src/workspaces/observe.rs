use jwst_codec::{Awareness, AwarenessEvent};

use super::*;

impl Workspace {
    pub async fn on_awareness_update(&mut self, f: impl Fn(&Awareness, AwarenessEvent) + Send + Sync + 'static) {
        self.awareness.write().unwrap().on_update(f);
    }
}
