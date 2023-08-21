use super::*;
use jwst_codec::{Awareness, AwarenessEvent};

impl Workspace {
    pub async fn on_awareness_update(&mut self, f: impl Fn(&Awareness, AwarenessEvent) + Send + Sync + 'static) {
        self.awareness.write().await.on_update(f);
    }
}
