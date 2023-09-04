use jwst_core::{Block as JwstBlock, Workspace as JwstWorkspace};
use wasm_bindgen::{prelude::*, JsCast};

cfg_if::cfg_if! {
    if #[cfg(feature = "log_panic")] {
        #[wasm_bindgen(js_name = setPanicHook)]
        pub fn set_panic_hook() {
            console_error_panic_hook::set_once();
        }
    }
}

#[wasm_bindgen]
pub struct Workspace(JwstWorkspace);

#[wasm_bindgen]
impl Workspace {
    #[wasm_bindgen(constructor)]
    pub fn new(id: String) -> Self {
        Self(JwstWorkspace::new(id).unwrap())
    }

    #[wasm_bindgen(js_name = clientId, getter)]
    pub fn client_id(&self) -> f64 {
        self.0.client_id() as f64
    }

    /// Create and return a `Block` instance
    #[wasm_bindgen]
    pub fn create(&mut self, block_id: String, flavour: String) -> Block {
        Block(
            self.0
                .get_blocks()
                .and_then(|mut s| s.create(block_id, &flavour))
                .unwrap(),
        )
    }

    /// Return a `Block` instance if block exists
    #[wasm_bindgen]
    pub fn get(mut self, block_id: String) -> Option<Block> {
        self.0.get_blocks().ok().and_then(|s| s.get(block_id).map(Block))
    }

    /// Check is a block exists
    #[wasm_bindgen]
    pub fn exists(&mut self, block_id: String) -> bool {
        self.0.get_blocks().map(|s| s.exists(&block_id)).unwrap_or_default()
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
