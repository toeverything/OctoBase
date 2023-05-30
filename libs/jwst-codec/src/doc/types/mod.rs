mod text;
mod traits;

use std::{
    collections::{hash_map::Entry, HashMap},
    sync::{Arc, RwLock},
};

pub use text::*;
pub use traits::*;

use crate::{JwstCodecError, JwstCodecResult};

use super::{
    store::{StoreRef, WeakStoreRef},
    StructInfo,
};

#[derive(Debug, Default)]
pub(crate) struct YTypeStore {
    pub store: WeakStoreRef,
    pub item: Option<StructInfo>,
    pub start: Option<StructInfo>,
    pub map: Option<HashMap<String, StructInfo>>,
    pub item_len: u64,
    pub content_len: u64,
    /// The tag name of XMLElement and XMLHook type
    pub name: Option<String>,
    /// The name of the type that directly belongs the store.
    pub root_name: Option<String>,
    pub kind: YTypeKind,
}

pub(crate) struct YTypeBuilder {
    store: StoreRef,
    /// The tag name of XMLElement and XMLHook type
    name: Option<String>,
    /// The name of the type that directly belongs the store.
    root_name: Option<String>,
    kind: YTypeKind,
}

impl YTypeBuilder {
    pub fn new(store: StoreRef) -> Self {
        Self {
            store,
            name: None,
            root_name: None,
            kind: YTypeKind::Unknown,
        }
    }

    pub fn with_kind(mut self, kind: YTypeKind) -> Self {
        self.kind = kind;

        self
    }

    pub fn set_name(mut self, name: String) -> Self {
        self.root_name = Some(name);

        self
    }

    pub fn set_tag_name(mut self, tag_name: String) -> Self {
        self.name = Some(tag_name);

        self
    }

    pub fn build<T: TryFrom<YType, Error = JwstCodecError>>(self) -> JwstCodecResult<T> {
        let ty = if let Some(root_name) = self.root_name {
            let store = self.store.write().unwrap();
            let mut types = store.types.write().unwrap();
            match types.entry(root_name.clone()) {
                Entry::Occupied(e) => e.get().clone(),
                Entry::Vacant(e) => {
                    let y_type: YType = YTypeStore {
                        kind: self.kind,
                        name: self.name,
                        root_name: Some(root_name),
                        store: Arc::downgrade(&self.store),
                        ..Default::default()
                    }
                    .into();

                    e.insert(y_type.clone());

                    y_type
                }
            }
        } else {
            YTypeStore {
                kind: self.kind,
                name: self.name,
                root_name: self.root_name.clone(),
                store: Arc::downgrade(&self.store),
                ..YTypeStore::default()
            }
            .into()
        };

        T::try_from(ty)
    }
}

#[macro_export(local_inner_macros)]
macro_rules! impl_variants {
    ({$($name: ident: $id: literal),*}) => {
        #[derive(Debug, Clone, Copy, PartialEq, Default)]
        pub enum YTypeKind {
            $($name,)*
            #[default]
            Unknown,
        }

        impl YTypeKind {
            pub fn as_str(&self) -> &'static str {
                match self {
                    $(YTypeKind::$name => std::stringify!($name),)*
                    YTypeKind::Unknown => "Unknown",
                }
            }
        }

        impl From<u64> for YTypeKind {
            fn from(value: u64) -> Self {
                match value {
                    $($id => YTypeKind::$name,)*
                    _ => YTypeKind::Unknown,
                }
            }
        }


        impl From<YTypeKind> for u64 {
            fn from(value: YTypeKind) -> Self {
                std::debug_assert!(value != YTypeKind::Unknown);
                match value {
                    $(YTypeKind::$name => $id,)*
                    _ => std::unreachable!(),
                }
            }
        }

        #[derive(Debug, Clone)]
        pub enum YType {
            $($name($name),)*
            Unknown(Unknown),
        }

        impl From<YTypeStore> for YType {
            fn from(value: YTypeStore) -> Self {
                match value.kind {
                    $(YTypeKind::$name => YType::$name($name(Arc::new(RwLock::new(value)))),)*
                    YTypeKind::Unknown => YType::Unknown(Unknown(Arc::new(RwLock::new(value)))),
                }
            }
        }

        impl YType {
            pub(crate) fn new(kind: YTypeKind, tag_name: Option<String>) -> Self {
                let inner = YTypeStore {
                    kind,
                    name: tag_name,
                    ..YTypeStore::default()
                };

                inner.into()
            }

            pub fn kind(&self) -> YTypeKind {
                match self {
                    $(YType::$name(_) => YTypeKind::$name,)*
                    YType::Unknown(_) => YTypeKind::Unknown,
                }
            }

            pub(crate) fn set_kind(&mut self, kind: YTypeKind) -> JwstCodecResult {
                std::debug_assert!(kind != YTypeKind::Unknown);

                if self.kind() != kind {
                    if let YType::Unknown(t) = self {
                        *self = match kind {
                            $(YTypeKind::$name => YType::$name($name::try_from(t.clone())?),)*
                            _ => std::unreachable!(),
                        }
                    } else {
                        return Err(JwstCodecError::TypeCastError(kind.as_str()));
                    }
                }

                Ok(())
            }

            pub(crate) fn read(&self) -> std::sync::RwLockReadGuard<YTypeStore> {
                match self {
                    $(YType::$name(t) => t.read()),*,
                    YType::Unknown(t) => t.0.read().unwrap(),
                }
            }

            pub(crate) fn write(&self) -> std::sync::RwLockWriteGuard<YTypeStore> {
                match self {
                    $(YType::$name(t) => t.write()),*,
                    YType::Unknown(t) => t.0.write().unwrap(),
                }
            }
        }
    };
}

#[macro_export(local_inner_macros)]
macro_rules! impl_type {
    ($name: ident) => {
        #[derive(Debug, Clone, Default)]
        pub struct $name(pub(crate) std::sync::Arc<std::sync::RwLock<super::YTypeStore>>);
        unsafe impl Sync for $name {}
        unsafe impl Send for $name {}

        impl $name {
            pub(crate) fn read(&self) -> std::sync::RwLockReadGuard<super::YTypeStore> {
                self.0.read().unwrap()
            }

            pub(crate) fn write(&self) -> std::sync::RwLockWriteGuard<super::YTypeStore> {
                self.0.write().unwrap()
            }
        }

        impl TryFrom<$crate::doc::types::YType> for $name {
            type Error = $crate::JwstCodecError;

            fn try_from(value: $crate::doc::types::YType) -> Result<Self, Self::Error> {
                match value {
                    $crate::doc::types::YType::$name(t) => Ok(t),
                    $crate::doc::types::YType::Unknown(t) => $name::try_from(t),
                    _ => Err($crate::JwstCodecError::TypeCastError("YText")),
                }
            }
        }

        impl TryFrom<$crate::doc::types::Unknown> for $name {
            type Error = $crate::JwstCodecError;

            fn try_from(value: $crate::doc::types::Unknown) -> Result<Self, Self::Error> {
                value.0.write().unwrap().kind = $crate::doc::types::YTypeKind::$name;
                Ok(Self(value.0))
            }
        }

        impl From<$name> for $crate::doc::types::YType {
            fn from(value: $name) -> Self {
                $crate::doc::types::YType::$name(value)
            }
        }
    };
}

impl_variants!({
    Array: 0,
    Map: 1,
    Text: 2,
    XMLElement: 3,
    XMLFragment: 4,
    XMLHook: 5,
    XMLText: 6
    // Doc: 9?
});

impl Default for YType {
    fn default() -> Self {
        YType::Unknown(Unknown::default())
    }
}

impl PartialEq for YType {
    fn eq(&self, other: &Self) -> bool {
        let store1 = self.read();
        let store2 = other.read();

        store1.item == store2.item
            || store1.root_name == store2.root_name
            || (store1.start.is_some() && store1.start == store2.start)
            || (store1.map.is_some() && store1.map == store2.map)
    }
}

#[derive(Debug, Clone, Default)]
pub struct Unknown(Arc<RwLock<YTypeStore>>);
unsafe impl Sync for Unknown {}
unsafe impl Send for Unknown {}

// TODO: move to separated impl files.
impl_type!(Array);
impl_type!(Map);
impl_type!(XMLElement);
impl_type!(XMLFragment);
impl_type!(XMLText);
impl_type!(XMLHook);
