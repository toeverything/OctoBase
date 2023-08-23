mod array;
mod list;
mod map;
mod text;

use std::{
    collections::{hash_map::Entry, HashMap},
    sync::Weak,
};

pub use array::*;
use list::*;
pub use map::*;
pub use text::*;

use super::{
    store::{StoreRef, WeakStoreRef},
    Node, *,
};
use crate::{
    sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard},
    Item, JwstCodecError, JwstCodecResult,
};

#[derive(Debug, Default)]
pub(crate) struct YType {
    pub start: Somr<Item>,
    pub item: Somr<Item>,
    pub map: Option<HashMap<String, Node>>,
    pub len: u64,
    /// The tag name of XMLElement and XMLHook type
    pub name: Option<String>,
    /// The name of the type that directly belongs the store.
    pub root_name: Option<String>,
    kind: YTypeKind,
    pub markers: Option<MarkerList>,
}

#[derive(Debug, Default, Clone)]
pub(crate) struct YTypeRef {
    pub store: WeakStoreRef,
    pub inner: Somr<RwLock<YType>>,
}

impl PartialEq for YType {
    fn eq(&self, other: &Self) -> bool {
        self.root_name == other.root_name
            || (self.start.is_some() && self.start == other.start)
            || (self.map.is_some() && self.map == other.map)
    }
}

impl PartialEq for YTypeRef {
    fn eq(&self, other: &Self) -> bool {
        self.inner.ptr_eq(&other.inner)
            || match (self.ty(), other.ty()) {
                (Some(l), Some(r)) => *l == *r,
                (None, None) => true,
                _ => false,
            }
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
}

impl YTypeRef {
    pub fn new(kind: YTypeKind, tag_name: Option<String>) -> Self {
        Self {
            inner: Somr::new(RwLock::new(YType::new(kind, tag_name))),
            store: Weak::new(),
        }
    }

    pub fn ty(&self) -> Option<RwLockReadGuard<YType>> {
        self.inner.get().and_then(|ty| ty.read().ok())
    }

    pub fn ty_mut(&self) -> Option<RwLockWriteGuard<YType>> {
        self.inner.get().and_then(|ty| ty.write().ok())
    }

    pub fn store<'a>(&self) -> Option<RwLockReadGuard<'a, DocStore>> {
        if let Some(store) = self.store.upgrade() {
            let ptr = unsafe { &*Arc::as_ptr(&store) };

            Some(ptr.read().unwrap())
        } else {
            None
        }
    }

    pub fn store_mut<'a>(&self) -> Option<RwLockWriteGuard<'a, DocStore>> {
        if let Some(store) = self.store.upgrade() {
            let ptr = unsafe { &*Arc::as_ptr(&store) };

            Some(ptr.write().unwrap())
        } else {
            None
        }
    }

    pub fn read(&self) -> Option<(RwLockReadGuard<DocStore>, RwLockReadGuard<YType>)> {
        self.store().and_then(|store| self.ty().map(|ty| (store, ty)))
    }

    pub fn write(&self) -> Option<(RwLockWriteGuard<DocStore>, RwLockWriteGuard<YType>)> {
        self.store_mut().and_then(|store| self.ty_mut().map(|ty| (store, ty)))
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

    pub fn build_exists<T: TryFrom<YTypeRef, Error = JwstCodecError>>(self) -> JwstCodecResult<T> {
        let store = self.store.read().unwrap();
        let ty = if let Some(root_name) = self.root_name {
            match store.types.get(&root_name) {
                Some(ty) => ty.clone(),
                None => {
                    return Err(JwstCodecError::RootStructNotFound(root_name));
                }
            }
        } else {
            return Err(JwstCodecError::TypeCastError("root_name is not set"));
        };

        drop(store);

        T::try_from(ty)
    }

    pub fn build<T: TryFrom<YTypeRef, Error = JwstCodecError>>(self) -> JwstCodecResult<T> {
        let mut store = self.store.write().unwrap();
        let ty = if let Some(root_name) = self.root_name {
            match store.types.entry(root_name.clone()) {
                Entry::Occupied(e) => e.get().clone(),
                Entry::Vacant(e) => {
                    let inner = Somr::new(RwLock::new(YType {
                        kind: self.kind,
                        name: self.name,
                        root_name: Some(root_name),
                        markers: Self::markers(self.kind),
                        ..Default::default()
                    }));

                    let ty = YTypeRef {
                        store: Arc::downgrade(&self.store),
                        inner,
                    };

                    let ty_ref = ty.clone();
                    e.insert(ty);

                    ty_ref
                }
            }
        } else {
            let inner = Somr::new(RwLock::new(YType {
                kind: self.kind,
                name: self.name,
                root_name: self.root_name.clone(),
                markers: Self::markers(self.kind),
                ..Default::default()
            }));

            let ty = YTypeRef {
                store: Arc::downgrade(&self.store),
                inner,
            };

            let ty_ref = ty.clone();

            store.dangling_types.insert(ty.inner.ptr().as_ptr() as usize, ty);

            ty_ref
        };

        drop(store);

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


        $(
            impl From<$name> for super::Value {
                fn from(value: $name) -> Self {
                    Self::$name(value)
                }
            }
        )*
    };
}

pub(crate) trait AsInner {
    type Inner;
    fn as_inner(&self) -> &Self::Inner;
}

#[macro_export(local_inner_macros)]
macro_rules! impl_type {
    ($name: ident) => {
        #[derive(Debug, Clone, PartialEq)]
        pub struct $name(pub(crate) super::YTypeRef);
        unsafe impl Sync for $name {}
        unsafe impl Send for $name {}

        impl $name {
            pub(crate) fn new(inner: super::YTypeRef) -> Self {
                Self(inner)
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
                if let Some((_, mut inner)) = value.write() {
                    match inner.kind {
                        super::YTypeKind::$name => Ok($name::new(value.clone())),
                        super::YTypeKind::Unknown => {
                            inner.set_kind(super::YTypeKind::$name)?;
                            Ok($name::new(value.clone()))
                        }
                        _ => Err($crate::JwstCodecError::TypeCastError(std::stringify!($name))),
                    }
                } else {
                    Err($crate::JwstCodecError::TypeCastError(std::stringify!($name)))
                }
            }
        }

        impl $name {
            pub(crate) fn from_unchecked(value: super::YTypeRef) -> Self {
                $name::new(value.clone())
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
impl_type!(XMLElement);
impl_type!(XMLFragment);
impl_type!(XMLText);
impl_type!(XMLHook);

#[derive(Debug, PartialEq)]
pub enum Value {
    Any(Any),
    Doc(Doc),
    Array(Array),
    Map(Map),
    Text(Text),
    XMLElement(XMLElement),
    XMLFragment(XMLFragment),
    XMLHook(XMLHook),
    XMLText(XMLText),
}

impl Value {
    pub fn to_any(&self) -> Option<Any> {
        match self {
            Value::Any(any) => Some(any.clone()),
            _ => None,
        }
    }

    pub fn to_array(&self) -> Option<Array> {
        match self {
            Value::Array(array) => Some(array.clone()),
            _ => None,
        }
    }

    pub fn to_map(&self) -> Option<Map> {
        match self {
            Value::Map(map) => Some(map.clone()),
            _ => None,
        }
    }

    pub fn to_text(&self) -> Option<Text> {
        match self {
            Value::Text(text) => Some(text.clone()),
            _ => None,
        }
    }

    pub fn from_vec<T: Into<Any>>(el: Vec<T>) -> Self {
        Value::Any(Any::Array(el.into_iter().map(|item| item.into()).collect::<Vec<_>>()))
    }
}

impl TryFrom<&Content> for Value {
    type Error = JwstCodecError;
    fn try_from(value: &Content) -> Result<Self, Self::Error> {
        Ok(match value {
            Content::Any(any) => Value::Any(if any.len() == 1 {
                any[0].clone()
            } else {
                Any::Array(any.clone())
            }),
            Content::String(s) => Value::Any(Any::String(s.clone())),
            Content::Json(json) => Value::Any(Any::Array(
                json.iter()
                    .map(|item| {
                        if let Some(s) = item {
                            Any::String(s.clone())
                        } else {
                            Any::Undefined
                        }
                    })
                    .collect::<Vec<_>>(),
            )),
            Content::Binary(buf) => Value::Any(Any::Binary(buf.clone())),
            Content::Embed(v) => Value::Any(v.clone()),
            Content::Type(ty) => match ty.ty().unwrap().kind {
                YTypeKind::Array => Value::Array(Array::from_unchecked(ty.clone())),
                YTypeKind::Map => Value::Map(Map::from_unchecked(ty.clone())),
                YTypeKind::Text => Value::Text(Text::from_unchecked(ty.clone())),
                YTypeKind::XMLElement => Value::XMLElement(XMLElement::from_unchecked(ty.clone())),
                YTypeKind::XMLFragment => Value::XMLFragment(XMLFragment::from_unchecked(ty.clone())),
                YTypeKind::XMLHook => Value::XMLHook(XMLHook::from_unchecked(ty.clone())),
                YTypeKind::XMLText => Value::XMLText(XMLText::from_unchecked(ty.clone())),
                // actually unreachable
                YTypeKind::Unknown => return Err(JwstCodecError::TypeCastError("unknown")),
            },
            Content::Doc { .. } => return Err(JwstCodecError::TypeCastError("unimplemented: Doc")),
            Content::Format { .. } => return Err(JwstCodecError::TypeCastError("unimplemented: Format")),
            Content::Deleted(_) => return Err(JwstCodecError::TypeCastError("unimplemented: Deleted")),
        })
    }
}

impl From<Value> for Content {
    fn from(value: Value) -> Self {
        match value {
            Value::Any(any) => Content::from(any),
            Value::Doc(doc) => Content::Doc {
                guid: doc.guid().to_owned(),
                // TODO: replace doc options if we got ones
                opts: Any::Undefined,
            },
            Value::Array(v) => Content::Type(v.0),
            Value::Map(v) => Content::Type(v.0),
            Value::Text(v) => Content::Type(v.0),
            Value::XMLElement(v) => Content::Type(v.0),
            Value::XMLFragment(v) => Content::Type(v.0),
            Value::XMLHook(v) => Content::Type(v.0),
            Value::XMLText(v) => Content::Type(v.0),
        }
    }
}

impl<T: Into<Any>> From<T> for Value {
    fn from(value: T) -> Self {
        Value::Any(value.into())
    }
}

impl From<Doc> for Value {
    fn from(value: Doc) -> Self {
        Value::Doc(value)
    }
}

impl ToString for Value {
    fn to_string(&self) -> String {
        match self {
            Value::Any(any) => any.to_string(),
            Value::Text(text) => text.to_string(),
            _ => String::default(),
        }
    }
}

impl serde::Serialize for Value {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Self::Any(any) => any.serialize(serializer),
            Self::Array(array) => array.serialize(serializer),
            Self::Map(map) => map.serialize(serializer),
            Self::Text(text) => text.serialize(serializer),
            // Self::XMLElement(xml_element) => xml_element.serialize(serializer),
            // Self::XMLFragment(xml_fragment) => xml_fragment.serialize(serializer),
            // Self::XMLHook(xml_hook) => xml_hook.serialize(serializer),
            // Self::XMLText(xml_text) => xml_text.serialize(serializer),
            // Self::Doc(doc) => doc.serialize(serializer),
            _ => serializer.serialize_none(),
        }
    }
}
