use super::{DynamicValue, Transaction};
use jwst::Block as JwstBlock;
use yrs::ReadTxn;

pub struct Block {
    block: JwstBlock,
}

impl Block {
    pub fn new(block: JwstBlock) -> Self {
        Self { block }
    }

    pub fn get(&self, trx: Transaction<'_>, key: String) -> Option<DynamicValue> {
        self.block.get(&trx.trx, &key).map(DynamicValue::new)
    }
}
