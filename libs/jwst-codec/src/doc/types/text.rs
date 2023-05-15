use crate::{wrap_inner, Item};

pub struct YTextInner {
    item: Item,
}

wrap_inner!(YText, YTextInner);
