use super::DynamicValue;
use jwst::Block as JwstBlock;

pub struct Block {
    block: JwstBlock,
}

impl Block {
    pub fn new(block: JwstBlock) -> Self {
        Self { block }
    }

    pub fn get(&self, key: String) -> Option<DynamicValue> {
        self.block.get(&key).map(DynamicValue::new)
    }
}
