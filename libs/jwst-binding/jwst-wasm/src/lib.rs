use js_sys::Uint8Array;
use jwst::{Block as JwstBlock, Workspace as JwstWorkspace};
use wasm_bindgen::{prelude::*, JsCast};
use yrs::{Subscription as YrsSubscription, UpdateEvent};

cfg_if::cfg_if! {
    if #[cfg(feature = "custom_alloc")] {
        #[global_allocator]
        static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;
    }
}
cfg_if::cfg_if! {
    if #[cfg(feature = "log_panic")] {
        #[wasm_bindgen(js_name = setPanicHook)]
        pub fn set_panic_hook() {
            console_error_panic_hook::set_once();
        }
    }
}

#[wasm_bindgen]
pub struct Subscription(YrsSubscription<UpdateEvent>);

#[wasm_bindgen]
pub struct Workspace(JwstWorkspace);

#[wasm_bindgen]
impl Workspace {
    #[wasm_bindgen(constructor)]
    pub fn new(id: String) -> Self {
        Self(JwstWorkspace::new(id))
    }

    #[wasm_bindgen(js_name = clientId, getter)]
    pub fn client_id(&self) -> f64 {
        self.0.client_id() as f64
    }

    /// Create and return a `Block` instance
    #[wasm_bindgen]
    pub fn create(&self, block_id: String, flavour: String) -> Block {
        Block(self.0.with_trx(|mut t| t.create(block_id, &flavour)))
    }

    /// Return a `Block` instance if block exists
    #[wasm_bindgen]
    pub fn get(&self, block_id: String) -> Option<Block> {
        self.0.get(block_id).map(|b| Block(b))
    }

    /// Check is a block exists
    #[wasm_bindgen]
    pub fn exists(&self, block_id: String) -> bool {
        self.0.exists(&block_id)
    }

    #[wasm_bindgen]
    pub fn observe(&mut self, f: js_sys::Function) -> Subscription {
        Subscription(self.0.observe(move |_, u| {
            let update = Uint8Array::from(u.update.as_slice());
            f.call1(&JsValue::UNDEFINED, &update).unwrap();
        }))
    }
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(typescript_type = "Array<String>")]
    pub type VecString;
}

#[wasm_bindgen]
pub struct Block(JwstBlock);

#[wasm_bindgen]
impl Block {
    #[wasm_bindgen(js_name = children, getter)]
    pub fn client_id(&self) -> VecString {
        self.0
            .children()
            .into_iter()
            .map(JsValue::from)
            .collect::<js_sys::Array>()
            .unchecked_into::<VecString>()
    }
}
