mod traits;

use std::{cell::RefCell, collections::HashMap, sync::Arc};

pub use traits::*;

use super::{StructRef, YType};

#[macro_export]
macro_rules! wrap_inner {
    ( $(($outer: ident, $inner: ident)),* ) => {
        $(
        pub struct $outer(Box<self::$inner>);

        impl  std::ops::Deref for $outer {
            type Target = self::$inner;

            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }

        impl  std::ops::DerefMut for $outer {
            fn deref_mut(&mut self) -> &mut Self::Target {
                &mut self.0
            }
        }
      )*
    };

  ( $outer: ident, $inner: ident ) => {
    wrap_inner!(($outer, $inner));
  };
}

pub enum TypeStoreKind {
    Array,
    Map,
    Text,
    XMLElement,
    XMLFragment,
    XMLText,
    Unknown,
}

pub struct TypeStore {
    pub start: Option<StructRef>,
    pub map: HashMap<String, StructRef>,
    pub item: Option<StructRef>,
    pub len: u64,
    kind: TypeStoreKind,
}

pub type TypeStoreRef = Arc<RefCell<TypeStore>>;

impl TypeStore {
    pub fn new(kind: TypeStoreKind) -> Self {
        Self {
            start: None,
            map: HashMap::new(),
            item: None,
            len: 0,
            kind,
        }
    }

    pub fn set_kind(&mut self, kind: TypeStoreKind) {
        if let TypeStoreKind::Unknown = self.kind {
            self.kind = kind;
        }
    }
}

impl From<TypeStore> for TypeStoreRef {
    fn from(value: TypeStore) -> Self {
        Arc::new(RefCell::new(value))
    }
}

impl From<YType> for TypeStore {
    fn from(value: YType) -> Self {
        let ty = match value {
            YType::Array => TypeStoreKind::Array,
            YType::Map => TypeStoreKind::Map,
            YType::Text => TypeStoreKind::Text,
            YType::XmlElement(_) => TypeStoreKind::XMLElement,
            YType::XmlText => TypeStoreKind::XMLText,
            YType::XmlFragment => TypeStoreKind::XMLFragment,
            _ => TypeStoreKind::Unknown,
        };

        Self {
            start: None,
            map: HashMap::new(),
            item: None,
            len: 0,
            kind: ty,
        }
    }
}
