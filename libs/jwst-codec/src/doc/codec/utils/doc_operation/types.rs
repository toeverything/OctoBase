use yrs::{ArrayRef, MapRef, TextRef, XmlElementRef, XmlFragmentRef, XmlTextRef};

#[derive(Hash, PartialEq, Eq, Clone, Debug, arbitrary::Arbitrary)]
pub enum OpType {
    HandleCurrent,
    CreateCRDTNestType,
}

#[derive(Hash, PartialEq, Eq, Clone, Debug, arbitrary::Arbitrary)]
pub enum MapOpType {
    Insert,
    Remove,
    Clear,
}

#[derive(PartialEq, Clone, Debug, arbitrary::Arbitrary)]
pub struct CRDTParam {
    pub op_type: OpType,
    pub new_nest_type: CRDTNestType,
    pub manipulate_source: ManipulateSource,
    pub insert_pos: InsertPos,
    pub key: String,
    pub value: String,
    pub map_op_type: MapOpType,
}

#[derive(Debug, Clone, PartialEq, arbitrary::Arbitrary)]
pub enum CRDTNestType {
    Array,
    Map,
    Text,
    XMLElement,
    XMLFragment,
    XMLText,
}

#[derive(Debug, Clone, PartialEq, arbitrary::Arbitrary)]
pub enum ManipulateSource {
    NewNestTypeFromYDocRoot,
    CurrentNestType,
    NewNestTypeFromCurrent,
}

#[derive(Debug, Clone, PartialEq, arbitrary::Arbitrary)]
pub enum InsertPos {
    BEGIN,
    MID,
    END,
}

#[derive(Clone)]
pub enum YrsNestType {
    ArrayType(ArrayRef),
    MapType(MapRef),
    TextType(TextRef),
    XMLElementType(XmlElementRef),
    XMLFragmentType(XmlFragmentRef),
    XMLTextType(XmlTextRef),
}
