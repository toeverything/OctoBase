mod text;
mod traits;

use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

pub use text::*;
pub use traits::*;

use crate::{JwstCodecError, JwstCodecResult};

use super::{store::WeakStoreRef, StructInfo};

#[derive(Debug, Default)]
pub(crate) struct YTypeStore {
    pub store: WeakStoreRef,
    pub item: Option<StructInfo>,
    pub start: Option<StructInfo>,
    pub map: Option<HashMap<String, StructInfo>>,
    pub item_len: u64,
    pub content_len: u64,
    pub tag_name: Option<String>,
    pub kind: YTypeKind,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum YTypeKind {
    Array,
    Map,
    Text,
    XMLElement,
    XMLFragment,
    XMLText,
    Unknown,
}

#[derive(Clone, Debug)]
pub enum YType {
    Text(Text),
    Array(Array),
    Map(Map),
    XMLElement(XMLElement),
    XMLFragment(XMLFragment),
    XMLText(XMLText),
    Unknown(Unknown),
}

impl Default for YType {
    fn default() -> Self {
        YType::Unknown(Unknown(Arc::new(RwLock::new(YTypeStore::default()))))
    }
}

impl YType {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub fn kind(&self) -> YTypeKind {
        match self {
            YType::Text(_) => YTypeKind::Text,
            YType::Array(_) => YTypeKind::Array,
            YType::Map(_) => YTypeKind::Map,
            YType::XMLElement(_) => YTypeKind::XMLElement,
            YType::XMLFragment(_) => YTypeKind::XMLFragment,
            YType::XMLText(_) => YTypeKind::XMLText,
            YType::Unknown(_) => YTypeKind::Unknown,
        }
    }

    pub(crate) fn set_kind(&mut self, kind: YTypeKind) -> JwstCodecResult {
        debug_assert!(kind != YTypeKind::Unknown);

        if self.kind() != kind {
            if let YType::Unknown(t) = self {
                *self = match kind {
                    YTypeKind::Text => YType::from(Text::try_from(t.clone())?),
                    YTypeKind::Array => YType::from(Array::try_from(t.clone())?),
                    YTypeKind::Map => YType::from(Map::try_from(t.clone())?),
                    YTypeKind::XMLElement => YType::from(XMLElement::try_from(t.clone())?),
                    YTypeKind::XMLFragment => YType::from(XMLFragment::try_from(t.clone())?),
                    YTypeKind::XMLText => YType::from(XMLText::try_from(t.clone())?),
                    _ => unreachable!(),
                }
            } else {
                return Err(JwstCodecError::TypeCastError(kind.as_str()));
            }
        }

        Ok(())
    }

    pub(crate) fn read(&self) -> std::sync::RwLockReadGuard<YTypeStore> {
        match self {
            YType::Text(t) => t.0.read().unwrap(),
            YType::Array(t) => t.0.read().unwrap(),
            YType::Map(t) => t.0.read().unwrap(),
            YType::XMLElement(t) => t.0.read().unwrap(),
            YType::XMLFragment(t) => t.0.read().unwrap(),
            YType::XMLText(t) => t.0.read().unwrap(),
            YType::Unknown(t) => t.0.read().unwrap(),
        }
    }

    pub(crate) fn write(&self) -> std::sync::RwLockWriteGuard<YTypeStore> {
        match self {
            YType::Text(t) => t.0.write().unwrap(),
            YType::Array(t) => t.0.write().unwrap(),
            YType::Map(t) => t.0.write().unwrap(),
            YType::XMLElement(t) => t.0.write().unwrap(),
            YType::XMLFragment(t) => t.0.write().unwrap(),
            YType::XMLText(t) => t.0.write().unwrap(),
            YType::Unknown(t) => t.0.write().unwrap(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Unknown(Arc<RwLock<YTypeStore>>);
unsafe impl Sync for Unknown {}
unsafe impl Send for Unknown {}

impl Default for YTypeKind {
    fn default() -> Self {
        YTypeKind::Unknown
    }
}

impl YTypeKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            YTypeKind::Array => "Array",
            YTypeKind::Map => "Map",
            YTypeKind::Text => "Text",
            YTypeKind::XMLElement => "XMLElement",
            YTypeKind::XMLFragment => "XMLFragment",
            YTypeKind::XMLText => "XMLText",
            YTypeKind::Unknown => "Unknown",
        }
    }
}

#[macro_export(local_inner_macros)]
macro_rules! impl_type {
    ($name: ident) => {
        #[derive(Debug, Clone, Default)]
        pub struct $name(pub(crate) std::sync::Arc<std::sync::RwLock<super::YTypeStore>>);
        unsafe impl Sync for $name {}
        unsafe impl Send for $name {}

        impl $name {
            pub(crate) fn new() -> Self {
                Default::default()
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

// TODO: move to separated impl files.
impl_type!(Array);
impl_type!(Map);
impl_type!(XMLElement);
impl_type!(XMLFragment);
impl_type!(XMLText);
