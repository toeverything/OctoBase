use super::*;
use jwst_codec::{Awareness, AwarenessEvent};
use nanoid::nanoid;
use std::{
    panic::{catch_unwind, AssertUnwindSafe},
    thread::sleep,
    time::Duration,
};
use yrs::{TransactionMut, UpdateEvent};

impl Workspace {
    pub async fn on_awareness_update(
        &mut self,
        f: impl Fn(&Awareness, AwarenessEvent) + Send + Sync + 'static,
    ) {
        self.awareness.write().await.on_update(f);
    }

    /// Subscribe to update events.
    pub fn observe(&mut self, f: impl Fn(&[u8]) + Clone + 'static) -> Option<String> {
        info!("workspace observe enter");
        let doc = self.doc();
        match catch_unwind(AssertUnwindSafe(move || {
            let mut retry = 10;
            let cb = move |update: &[u8]| {
                trace!("workspace observe: observe_update_v1, {:?}", &update);
                if let Err(e) = catch_unwind(AssertUnwindSafe(|| f(update))) {
                    error!("panic in observe callback: {:?}", e);
                }
            };
            doc.subscribe(cb.clone());
        })) {
            Ok(sub) => match sub {
                Ok(sub) => {
                    let id = nanoid!();
                    match self.sub.lock() {
                        Ok(mut guard) => {
                            guard.insert(id.clone(), sub);
                        }
                        Err(e) => {
                            error!("failed to lock sub: {:?}", e);
                        }
                    }
                    Some(id)
                }
                Err(e) => {
                    error!("failed to observe: {:?}", e);
                    None
                }
            },
            Err(e) => {
                error!("panic in observe callback: {:?}", e);
                None
            }
        }
    }
}
