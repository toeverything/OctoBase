pub mod array;
pub mod map;
pub mod text;
pub mod xml_element;
pub mod xml_fragment;
pub mod xml_text;

use super::*;
use std::collections::HashMap;
use yrs::{
    Array, ArrayPrelim, ArrayRef, Doc, GetString, Map, MapPrelim, MapRef, Text, TextPrelim,
    TextRef, Transact, XmlFragment, XmlTextPrelim,
};

use array::*;
use map::*;
use text::*;
use xml_element::*;
use xml_fragment::*;
use xml_text::*;

pub struct OpsRegistry(
    HashMap<CRDTNestType, HashMap<NestDataOpType, Box<dyn Fn(&yrs::Doc, &YrsNestType, CRDTParam)>>>,
);

impl OpsRegistry {
    pub fn new() -> Self {
        let mut map = HashMap::new();
        map.insert(CRDTNestType::Map, gen_map_ref_ops());
        map.insert(CRDTNestType::Array, gen_array_ref_ops());
        map.insert(CRDTNestType::Text, gen_text_ref_ops());
        map.insert(CRDTNestType::XMLText, gen_xml_text_ref_ops());
        map.insert(CRDTNestType::XMLElement, gen_xml_element_ref_ops());
        map.insert(CRDTNestType::XMLFragment, gen_xml_fragment_ref_ops());

        OpsRegistry(map)
    }

    pub fn get_ops(
        &self,
        crdt_nest_type: &CRDTNestType,
    ) -> &HashMap<NestDataOpType, Box<dyn Fn(&yrs::Doc, &YrsNestType, CRDTParam)>> {
        match crdt_nest_type {
            CRDTNestType::Array => self.0.get(&CRDTNestType::Array).unwrap(),
            CRDTNestType::Map => self.0.get(&CRDTNestType::Map).unwrap(),
            CRDTNestType::Text => self.0.get(&CRDTNestType::Text).unwrap(),
            CRDTNestType::XMLText => self.0.get(&CRDTNestType::XMLText).unwrap(),
            CRDTNestType::XMLElement => self.0.get(&CRDTNestType::XMLElement).unwrap(),
            CRDTNestType::XMLFragment => self.0.get(&CRDTNestType::XMLFragment).unwrap(),
        }
    }

    pub fn get_ops_from_yrs_nest_type(
        &self,
        yrs_nest_type: &YrsNestType,
    ) -> &HashMap<NestDataOpType, Box<dyn Fn(&yrs::Doc, &YrsNestType, CRDTParam)>> {
        match yrs_nest_type {
            YrsNestType::ArrayType(_) => self.get_ops(&CRDTNestType::Array),
            YrsNestType::MapType(_) => self.get_ops(&CRDTNestType::Map),
            YrsNestType::TextType(_) => self.get_ops(&CRDTNestType::Text),
            YrsNestType::XMLTextType(_) => self.get_ops(&CRDTNestType::XMLText),
            YrsNestType::XMLElementType(_) => self.get_ops(&CRDTNestType::XMLElement),
            YrsNestType::XMLFragmentType(_) => self.get_ops(&CRDTNestType::XMLFragment),
        }
    }

    pub fn operate_yrs_nest_type(
        &self,
        doc: &yrs::Doc,
        cur_crdt_nest_type: YrsNestType,
        crdt_param: CRDTParam,
    ) {
        let ops = self.get_ops_from_yrs_nest_type(&cur_crdt_nest_type);
        ops.get(&crdt_param.nest_data_op_type).unwrap()(doc, &cur_crdt_nest_type, crdt_param)
    }
}

pub fn yrs_create_nest_type_from_root(
    doc: &yrs::Doc,
    target_type: CRDTNestType,
    key: &str,
) -> YrsNestType {
    match target_type {
        CRDTNestType::Array => YrsNestType::ArrayType(doc.get_or_insert_array(key)),
        CRDTNestType::Map => YrsNestType::MapType(doc.get_or_insert_map(key)),
        CRDTNestType::Text => YrsNestType::TextType(doc.get_or_insert_text(key)),
        CRDTNestType::XMLElement => YrsNestType::XMLElementType(doc.get_or_insert_xml_element(key)),
        CRDTNestType::XMLFragment => {
            YrsNestType::XMLFragmentType(doc.get_or_insert_xml_fragment(key))
        }
        CRDTNestType::XMLText => YrsNestType::XMLTextType(doc.get_or_insert_xml_text(key)),
    }
}

pub fn gen_nest_type_from_root(doc: &mut Doc, crdt_param: &CRDTParam) -> Option<YrsNestType> {
    match crdt_param.new_nest_type {
        CRDTNestType::Array => Some(yrs_create_nest_type_from_root(
            &doc,
            CRDTNestType::Array,
            crdt_param.key.as_str(),
        )),
        CRDTNestType::Map => Some(yrs_create_nest_type_from_root(
            &doc,
            CRDTNestType::Map,
            crdt_param.key.as_str(),
        )),
        CRDTNestType::Text => Some(yrs_create_nest_type_from_root(
            &doc,
            CRDTNestType::Text,
            crdt_param.key.as_str(),
        )),
        CRDTNestType::XMLText => Some(yrs_create_nest_type_from_root(
            &doc,
            CRDTNestType::XMLText,
            crdt_param.key.as_str(),
        )),
        CRDTNestType::XMLElement => Some(yrs_create_nest_type_from_root(
            &doc,
            CRDTNestType::XMLElement,
            crdt_param.key.as_str(),
        )),
        CRDTNestType::XMLFragment => Some(yrs_create_nest_type_from_root(
            &doc,
            CRDTNestType::XMLFragment,
            crdt_param.key.as_str(),
        )),
    }
}

pub fn gen_nest_type_from_nest_type(
    doc: &mut Doc,
    crdt_param: CRDTParam,
    mut nest_type: &mut YrsNestType,
) -> Option<YrsNestType> {
    match crdt_param.new_nest_type {
        CRDTNestType::Array => match yrs_create_array_from_nest_type(
            &doc,
            &mut nest_type,
            &crdt_param.insert_pos,
            crdt_param.key,
        ) {
            Some(array_ref) => Some(YrsNestType::ArrayType(array_ref)),
            None => None,
        },
        CRDTNestType::Map => match yrs_create_map_from_nest_type(
            &doc,
            &mut nest_type,
            &crdt_param.insert_pos,
            crdt_param.key,
        ) {
            Some(map_ref) => Some(YrsNestType::MapType(map_ref)),
            None => None,
        },
        CRDTNestType::Text => match yrs_create_text_from_nest_type(
            &doc,
            &mut nest_type,
            &crdt_param.insert_pos,
            crdt_param.key,
        ) {
            Some(text_ref) => Some(YrsNestType::TextType(text_ref)),
            None => None,
        },
        _ => None,
    }
}

pub fn random_pick_num(len: u32, insert_pos: &InsertPos) -> u32 {
    match insert_pos {
        InsertPos::BEGIN => 0,
        InsertPos::MID => len / 2,
        InsertPos::END => len,
    }
}
