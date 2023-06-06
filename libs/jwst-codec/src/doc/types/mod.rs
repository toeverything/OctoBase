mod array;
mod list;
mod text;
mod traits;

use super::*;
use list::*;
use std::{
    collections::{hash_map::Entry, HashMap},
    sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard, Weak},
};

pub use array::*;
pub use text::*;
pub use traits::*;

use crate::{Item, JwstCodecError, JwstCodecResult};

use super::{
    store::{StoreRef, WeakStoreRef},
    StructInfo,
};

#[derive(Debug, Default)]
pub struct YType {
    pub store: WeakStoreRef,
    pub start: Option<StructInfo>,
    pub item: Option<Weak<Item>>,
    pub map: Option<HashMap<String, StructInfo>>,
    pub len: u64,
    /// The tag name of XMLElement and XMLHook type
    pub name: Option<String>,
    /// The name of the type that directly belongs the store.
    pub root_name: Option<String>,
    pub kind: YTypeKind,
    pub markers: Option<MarkerList>,
}

pub type YTypeRef = Arc<RwLock<YType>>;

impl PartialEq for YType {
    fn eq(&self, other: &Self) -> bool {
        self.root_name == other.root_name
            || (self.start.is_some() && self.start == other.start)
            || (self.map.is_some() && self.map == other.map)
    }
}

impl YType {
    pub fn new(kind: YTypeKind, tag_name: Option<String>) -> Self {
        YType {
            kind,
            name: tag_name,
            ..YType::default()
        }
    }

    pub fn into_ref(self) -> YTypeRef {
        Arc::new(RwLock::new(self))
    }

    pub fn kind(&self) -> YTypeKind {
        self.kind
    }

    pub fn set_kind(&mut self, kind: YTypeKind) -> JwstCodecResult {
        std::debug_assert!(kind != YTypeKind::Unknown);

        if self.kind() != kind {
            if self.kind == YTypeKind::Unknown {
                self.kind = kind;
            } else {
                return Err(JwstCodecError::TypeCastError(kind.as_str()));
            }
        }

        Ok(())
    }

    pub fn start(&self) -> Option<ItemRef> {
        self.start.as_ref().and_then(|s| s.as_item())
    }

    pub fn store<'a>(&self) -> Option<RwLockReadGuard<'a, DocStore>> {
        if let Some(store) = self.store.upgrade() {
            let ptr = unsafe { &*Arc::as_ptr(&store) };

            Some(ptr.read().unwrap())
        } else {
            None
        }
    }

    pub fn store_mut<'a>(&mut self) -> Option<RwLockWriteGuard<'a, DocStore>> {
        if let Some(store) = self.store.upgrade() {
            let ptr = unsafe { &*Arc::as_ptr(&store) };

            Some(ptr.write().unwrap())
        } else {
            None
        }
    }

    pub fn item(&self) -> Option<ItemRef> {
        self.item.as_ref().and_then(|i| i.upgrade())
    }
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

    #[allow(dead_code)]
    pub fn set_tag_name(mut self, tag_name: String) -> Self {
        self.name = Some(tag_name);

        self
    }

    pub fn build<T: TryFrom<YTypeRef, Error = JwstCodecError>>(self) -> JwstCodecResult<T> {
        let ty = if let Some(root_name) = self.root_name {
            let store = self.store.write().unwrap();
            let mut types = store.types.write().unwrap();
            match types.entry(root_name.clone()) {
                Entry::Occupied(e) => e.get().clone(),
                Entry::Vacant(e) => {
                    let y_type: Arc<RwLock<YType>> = Arc::new(RwLock::new(YType {
                        kind: self.kind,
                        name: self.name,
                        root_name: Some(root_name),
                        store: Arc::downgrade(&self.store),
                        markers: Self::markers(self.kind),
                        ..Default::default()
                    }));

                    e.insert(y_type.clone());

                    y_type
                }
            }
        } else {
            Arc::new(RwLock::new(YType {
                kind: self.kind,
                name: self.name,
                root_name: self.root_name.clone(),
                store: Arc::downgrade(&self.store),
                markers: Self::markers(self.kind),
                ..YType::default()
            }))
        };

        T::try_from(ty)
    }

    fn markers(kind: YTypeKind) -> Option<MarkerList> {
        match kind {
            YTypeKind::Map => None,
            _ => Some(MarkerList::new()),
        }
    }
}

#[macro_export(local_inner_macros)]
macro_rules! impl_variants {
    ({$($name: ident: $codec_ref: literal),*}) => {
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
                    $($codec_ref => YTypeKind::$name,)*
                    _ => YTypeKind::Unknown,
                }
            }
        }


        impl From<YTypeKind> for u64 {
            fn from(value: YTypeKind) -> Self {
                std::debug_assert!(value != YTypeKind::Unknown);
                match value {
                    $(YTypeKind::$name => $codec_ref,)*
                    _ => std::unreachable!(),
                }
            }
        }
    };
}

pub(crate) trait AsInner {
    type Inner;
    fn as_inner(&self) -> &Self::Inner;
}

#[macro_export(local_inner_macros)]
macro_rules! impl_type {
    ($name: ident) => {
        #[derive(Debug, Clone, Default)]
        pub struct $name(pub(crate) super::YTypeRef);
        unsafe impl Sync for $name {}
        unsafe impl Send for $name {}

        impl $name {
            pub(crate) fn new(inner: super::YTypeRef) -> Self {
                Self(inner)
            }

            #[allow(dead_code)]
            #[inline(always)]
            fn read(&self) -> std::sync::RwLockReadGuard<super::YType> {
                self.0.read().unwrap()
            }

            #[allow(dead_code)]
            #[inline(always)]
            fn write(&self) -> std::sync::RwLockWriteGuard<super::YType> {
                self.0.write().unwrap()
            }
        }

        impl super::AsInner for $name {
            type Inner = super::YTypeRef;

            #[inline(always)]
            fn as_inner(&self) -> &Self::Inner {
                &self.0
            }
        }

        impl TryFrom<super::YTypeRef> for $name {
            type Error = $crate::JwstCodecError;

            fn try_from(value: super::YTypeRef) -> Result<Self, Self::Error> {
                let mut inner = value.write().unwrap();
                match inner.kind {
                    super::YTypeKind::$name => Ok($name::new(value.clone())),
                    super::YTypeKind::Unknown => {
                        inner.set_kind(super::YTypeKind::$name)?;
                        Ok($name::new(value.clone()))
                    }
                    _ => Err($crate::JwstCodecError::TypeCastError("Text")),
                }
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

// TODO: move to separated impl files.
impl_type!(Array);
impl_type!(Map);
impl_type!(XMLElement);
impl_type!(XMLFragment);
impl_type!(XMLText);
impl_type!(XMLHook);
