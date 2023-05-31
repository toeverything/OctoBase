use super::*;
use std::collections::HashMap;
use yrs::{
    Array, ArrayPrelim, ArrayRef, Doc, Map, MapPrelim, MapRef, Text, TextPrelim, TextRef, Transact,
};

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

pub fn yrs_create_map_from_nest_type(
    doc: &yrs::Doc,
    current: &mut YrsNestType,
    insert_pos: &InsertPos,
    key: String,
) -> Option<MapRef> {
    let cal_index = |len: u32| -> u32 {
        match insert_pos {
            InsertPos::BEGIN => 0,
            InsertPos::MID => len / 2,
            InsertPos::END => len,
        }
    };
    let mut trx = doc.transact_mut();
    let map_prelim = MapPrelim::<String>::from(HashMap::new());
    match current {
        YrsNestType::ArrayType(array) => {
            let index = cal_index(array.len(&trx));
            Some(array.insert(&mut trx, index, map_prelim).unwrap())
        }
        YrsNestType::MapType(map) => Some(map.insert(&mut trx, key, map_prelim).unwrap()),
        YrsNestType::TextType(text) => {
            let index = cal_index(text.len(&trx));
            Some(text.insert_embed(&mut trx, index, map_prelim).unwrap())
        }
        YrsNestType::XMLTextType(xml_text) => {
            let index = cal_index(xml_text.len(&trx));
            Some(xml_text.insert_embed(&mut trx, index, map_prelim).unwrap())
        }
        _ => None,
    }
}

pub fn yrs_create_array_from_nest_type(
    doc: &yrs::Doc,
    current: &mut YrsNestType,
    insert_pos: &InsertPos,
    key: String,
) -> Option<ArrayRef> {
    let cal_index = |len: u32| -> u32 {
        match insert_pos {
            InsertPos::BEGIN => 0,
            InsertPos::MID => len / 2,
            InsertPos::END => len,
        }
    };
    let mut trx = doc.transact_mut();
    let array_prelim = ArrayPrelim::default();
    match current {
        YrsNestType::ArrayType(array) => {
            let index = cal_index(array.len(&trx));
            Some(array.insert(&mut trx, index, array_prelim).unwrap())
        }
        YrsNestType::MapType(map) => Some(map.insert(&mut trx, key, array_prelim).unwrap()),
        YrsNestType::TextType(text) => {
            let index = cal_index(text.len(&trx));
            Some(text.insert_embed(&mut trx, index, array_prelim).unwrap())
        }
        YrsNestType::XMLTextType(xml_text) => {
            let index = cal_index(xml_text.len(&trx));
            Some(
                xml_text
                    .insert_embed(&mut trx, index, array_prelim)
                    .unwrap(),
            )
        }
        _ => None,
    }
}

pub fn yrs_create_text_from_nest_type(
    doc: &yrs::Doc,
    current: &mut YrsNestType,
    insert_pos: &InsertPos,
    key: String,
) -> Option<TextRef> {
    let cal_index_closure = |len: u32| -> u32 { pick_num(len, insert_pos) };
    let mut trx = doc.transact_mut();
    let text_prelim = TextPrelim::new("");
    match current {
        YrsNestType::ArrayType(array) => {
            let index = cal_index_closure(array.len(&trx));
            Some(array.insert(&mut trx, index, text_prelim).unwrap())
        }
        YrsNestType::MapType(map) => Some(map.insert(&mut trx, key, text_prelim).unwrap()),
        YrsNestType::TextType(text) => {
            let index = cal_index_closure(text.len(&trx));
            Some(text.insert_embed(&mut trx, index, text_prelim).unwrap())
        }
        YrsNestType::XMLTextType(xml_text) => {
            let index = cal_index_closure(xml_text.len(&trx));
            Some(xml_text.insert_embed(&mut trx, index, text_prelim).unwrap())
        }
        _ => None,
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

pub fn pick_num(len: u32, insert_pos: &InsertPos) -> u32 {
    match insert_pos {
        InsertPos::BEGIN => 0,
        InsertPos::MID => len / 2,
        InsertPos::END => len,
    }
}

pub struct YrsMapOps {
    ops: HashMap<MapOpType, Box<dyn Fn(&yrs::Doc, &MapRef, CRDTParam)>>,
}

impl YrsMapOps {
    pub fn operate_map_ref(&self, doc: &yrs::Doc, map_ref: &mut MapRef, crdt_param: CRDTParam) {
        self.ops.get(&crdt_param.map_op_type).unwrap()(doc, map_ref, crdt_param);
    }

    pub fn new() -> Self {
        let mut ops: HashMap<MapOpType, Box<dyn Fn(&yrs::Doc, &MapRef, CRDTParam)>> =
            HashMap::new();

        let insert_op = |doc: &yrs::Doc, map: &MapRef, params: CRDTParam| {
            let mut trx = doc.transact_mut();
            map.insert(&mut trx, params.key, params.value).unwrap();
        };

        let remove_op = |doc: &yrs::Doc, map: &MapRef, params: CRDTParam| {
            let rand_key = {
                let trx = doc.transact_mut();
                let iter = map.iter(&trx);
                let len = map.len(&trx) as usize;
                let skip_step = if len <= 1 {
                    0
                } else {
                    pick_num((len - 1) as u32, &params.insert_pos)
                };

                match iter.skip(skip_step as usize).next() {
                    Some((key, _value)) => Some(key.to_string().clone()),
                    None => None,
                }
            };

            if let Some(key) = rand_key {
                let mut trx = doc.transact_mut();
                map.remove(&mut trx, &key).unwrap();
            }
        };

        let clear_op = |doc: &yrs::Doc, map: &MapRef, _params: CRDTParam| {
            let mut trx = doc.transact_mut();
            map.clear(&mut trx);
        };

        ops.insert(MapOpType::Insert, Box::new(insert_op));
        ops.insert(MapOpType::Remove, Box::new(remove_op));
        ops.insert(MapOpType::Clear, Box::new(clear_op));

        Self { ops }
    }
}
