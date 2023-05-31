use super::*;
use std::collections::HashMap;
use yrs::{Array, ArrayPrelim, ArrayRef, Doc, Map, MapPrelim, MapRef, Text, TextPrelim, TextRef, Transact, XmlFragment, XmlTextPrelim};

// use super::types::ArrayOpType;

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
    let cal_index_closure = |len: u32| -> u32 { random_pick_num(len, insert_pos) };
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

pub fn random_pick_num(len: u32, insert_pos: &InsertPos) -> u32 {
    match insert_pos {
        InsertPos::BEGIN => 0,
        InsertPos::MID => len / 2,
        InsertPos::END => len,
    }
}

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

pub fn gen_array_ref_ops(
) -> HashMap<NestDataOpType, Box<dyn Fn(&yrs::Doc, &YrsNestType, CRDTParam)>> {
    let mut ops: HashMap<NestDataOpType, Box<dyn Fn(&yrs::Doc, &YrsNestType, CRDTParam)>> =
        HashMap::new();

    let insert_op = |doc: &yrs::Doc, nest_input: &YrsNestType, params: CRDTParam| {
        let array = match nest_input {
            YrsNestType::ArrayType(array) => array,
            _ => unreachable!(),
        };
        let mut trx = doc.transact_mut();
        let len = array.len(&trx);
        let index = random_pick_num(len, &params.insert_pos);
        array.insert(&mut trx, index, params.value).unwrap();
    };

    let delete_op = |doc: &yrs::Doc, nest_input: &YrsNestType, params: CRDTParam| {
        let array = match nest_input {
            YrsNestType::ArrayType(array) => array,
            _ => unreachable!(),
        };
        let mut trx = doc.transact_mut();
        let len = array.len(&trx);
        if len >= 1 {
            let index = random_pick_num(len - 1, &params.insert_pos);
            array.remove(&mut trx, index).unwrap();
        }
    };

    let clear_op = |doc: &yrs::Doc, nest_input: &YrsNestType, _params: CRDTParam| {
        let array = match nest_input {
            YrsNestType::ArrayType(array) => array,
            _ => unreachable!(),
        };
        let mut trx = doc.transact_mut();
        let len = array.len(&trx);
        for _ in 0..len {
            array.remove(&mut trx, 0).unwrap();
        }
    };

    ops.insert(NestDataOpType::Insert, Box::new(insert_op));
    ops.insert(NestDataOpType::Delete, Box::new(delete_op));
    ops.insert(NestDataOpType::Clear, Box::new(clear_op));

    ops
}

pub fn gen_map_ref_ops() -> HashMap<NestDataOpType, Box<dyn Fn(&yrs::Doc, &YrsNestType, CRDTParam)>>
{
    let mut ops: HashMap<NestDataOpType, Box<dyn Fn(&yrs::Doc, &YrsNestType, CRDTParam)>> =
        HashMap::new();

    let insert_op = |doc: &yrs::Doc, nest_input: &YrsNestType, params: CRDTParam| {
        let map = match nest_input {
            YrsNestType::MapType(map) => map,
            _ => unreachable!(),
        };
        let mut trx = doc.transact_mut();
        map.insert(&mut trx, params.key, params.value).unwrap();
    };

    let remove_op = |doc: &yrs::Doc, nest_input: &YrsNestType, params: CRDTParam| {
        let map = match nest_input {
            YrsNestType::MapType(map) => map,
            _ => unreachable!(),
        };
        let rand_key = {
            let trx = doc.transact_mut();
            let iter = map.iter(&trx);
            let len = map.len(&trx) as usize;
            let skip_step = if len <= 1 {
                0
            } else {
                random_pick_num((len - 1) as u32, &params.insert_pos)
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

    let clear_op = |doc: &yrs::Doc, nest_input: &YrsNestType, _params: CRDTParam| {
        let map = match nest_input {
            YrsNestType::MapType(map) => map,
            _ => unreachable!(),
        };
        let mut trx = doc.transact_mut();
        map.clear(&mut trx);
    };

    ops.insert(NestDataOpType::Insert, Box::new(insert_op));
    ops.insert(NestDataOpType::Delete, Box::new(remove_op));
    ops.insert(NestDataOpType::Clear, Box::new(clear_op));

    ops
}

pub fn gen_text_ref_ops() -> HashMap<NestDataOpType, Box<dyn Fn(&yrs::Doc, &YrsNestType, CRDTParam)>>
{
    let mut ops: HashMap<NestDataOpType, Box<dyn Fn(&yrs::Doc, &YrsNestType, CRDTParam)>> =
        HashMap::new();

    let insert_op = |doc: &yrs::Doc, nest_input: &YrsNestType, params: CRDTParam| {
        let text = match nest_input {
            YrsNestType::TextType(text) => text,
            _ => unreachable!(),
        };
        let mut trx = doc.transact_mut();
        let len = text.len(&trx);
        let index = random_pick_num(len, &params.insert_pos);
        text.insert(&mut trx, index, &params.value).unwrap();
    };

    let remove_op = |doc: &yrs::Doc, nest_input: &YrsNestType, params: CRDTParam| {
        let text = match nest_input {
            YrsNestType::TextType(text) => text,
            _ => unreachable!(),
        };
        let mut trx = doc.transact_mut();
        let len = text.len(&trx);
        if len >= 1 {
            let index = random_pick_num(len - 1, &params.insert_pos);
            text.remove_range(&mut trx, index, 1).unwrap();
        }
    };

    let clear_op = |doc: &yrs::Doc, nest_input: &YrsNestType, _params: CRDTParam| {
        let text = match nest_input {
            YrsNestType::TextType(text) => text,
            _ => unreachable!(),
        };
        let mut trx = doc.transact_mut();
        let len = text.len(&trx);
        for _ in 0..len {
            text.remove_range(&mut trx, 0, 1).unwrap();
        }
    };

    ops.insert(NestDataOpType::Insert, Box::new(insert_op));
    ops.insert(NestDataOpType::Delete, Box::new(remove_op));
    ops.insert(NestDataOpType::Clear, Box::new(clear_op));

    ops
}

pub fn gen_xml_text_ref_ops(
) -> HashMap<NestDataOpType, Box<dyn Fn(&yrs::Doc, &YrsNestType, CRDTParam)>> {
    let mut ops: HashMap<NestDataOpType, Box<dyn Fn(&yrs::Doc, &YrsNestType, CRDTParam)>> =
        HashMap::new();

    let insert_op = |doc: &yrs::Doc, nest_input: &YrsNestType, params: CRDTParam| {
        let xml_text = match nest_input {
            YrsNestType::XMLTextType(xml_text) => xml_text,
            _ => unreachable!(),
        };
        let mut trx = doc.transact_mut();
        let len = xml_text.len(&trx);
        let index = random_pick_num(len, &params.insert_pos);
        xml_text.insert(&mut trx, index, &params.value).unwrap();
    };

    let remove_op = |doc: &yrs::Doc, nest_input: &YrsNestType, params: CRDTParam| {
        let xml_text = match nest_input {
            YrsNestType::XMLTextType(xml_text) => xml_text,
            _ => unreachable!(),
        };
        let mut trx = doc.transact_mut();
        let len = xml_text.len(&trx);
        if len >= 1 {
            let index = random_pick_num(len - 1, &params.insert_pos);
            xml_text.remove_range(&mut trx, index, 1).unwrap();
        }
    };

    let clear_op = |doc: &yrs::Doc, nest_input: &YrsNestType, _params: CRDTParam| {
        let xml_text = match nest_input {
            YrsNestType::XMLTextType(xml_text) => xml_text,
            _ => unreachable!(),
        };
        let mut trx = doc.transact_mut();
        let len = xml_text.len(&trx);
        for _ in 0..len {
            xml_text.remove_range(&mut trx, 0, 1).unwrap();
        }
    };

    ops.insert(NestDataOpType::Insert, Box::new(insert_op));
    ops.insert(NestDataOpType::Delete, Box::new(remove_op));
    ops.insert(NestDataOpType::Clear, Box::new(clear_op));

    ops
}

pub fn gen_xml_fragment_ref_ops(
) -> HashMap<NestDataOpType, Box<dyn Fn(&yrs::Doc, &YrsNestType, CRDTParam)>> {
    let mut ops: HashMap<NestDataOpType, Box<dyn Fn(&yrs::Doc, &YrsNestType, CRDTParam)>> =
        HashMap::new();

    let insert_op = |doc: &yrs::Doc, nest_input: &YrsNestType, params: CRDTParam| {
        let xml_fragment = match nest_input {
            YrsNestType::XMLFragmentType(xml_fragment) => xml_fragment,
            _ => unreachable!(),
        };
        let mut trx = doc.transact_mut();
        let len = xml_fragment.len(&trx);
        let index = random_pick_num(len, &params.insert_pos);
        xml_fragment.insert(&mut trx, index, XmlTextPrelim::new(params.value)).unwrap();
    };

    let remove_op = |doc: &yrs::Doc, nest_input: &YrsNestType, params: CRDTParam| {
        let xml_fragment = match nest_input {
            YrsNestType::XMLFragmentType(xml_fragment) => xml_fragment,
            _ => unreachable!(),
        };
        let mut trx = doc.transact_mut();
        let len = xml_fragment.len(&trx);
        if len >= 1 {
            let index = random_pick_num(len - 1, &params.insert_pos);
            xml_fragment.remove_range(&mut trx, index, 1).unwrap();
        }
    };

    let clear_op = |doc: &yrs::Doc, nest_input: &YrsNestType, _params: CRDTParam| {
        let xml_fragment = match nest_input {
            YrsNestType::XMLFragmentType(xml_fragment) => xml_fragment,
            _ => unreachable!(),
        };
        let mut trx = doc.transact_mut();
        let len = xml_fragment.len(&trx);
        for _ in 0..len {
            xml_fragment.remove_range(&mut trx, 0, 1).unwrap();
        }
    };

    ops.insert(NestDataOpType::Insert, Box::new(insert_op));
    ops.insert(NestDataOpType::Delete, Box::new(remove_op));
    ops.insert(NestDataOpType::Clear, Box::new(clear_op));

    ops
}

pub fn gen_xml_element_ref_ops(
) -> HashMap<NestDataOpType, Box<dyn Fn(&yrs::Doc, &YrsNestType, CRDTParam)>> {
    let mut ops: HashMap<NestDataOpType, Box<dyn Fn(&yrs::Doc, &YrsNestType, CRDTParam)>> =
        HashMap::new();

    let insert_op = |doc: &yrs::Doc, nest_input: &YrsNestType, params: CRDTParam| {
        let xml_element = match nest_input {
            YrsNestType::XMLElementType(xml_element) => xml_element,
            _ => unreachable!(),
        };
        let mut trx = doc.transact_mut();
        let len = xml_element.len(&trx);
        let index = random_pick_num(len, &params.insert_pos);
        xml_element.insert(&mut trx, index, XmlTextPrelim::new(params.value)).unwrap();
    };

    let remove_op = |doc: &yrs::Doc, nest_input: &YrsNestType, params: CRDTParam| {
        let xml_element = match nest_input {
            YrsNestType::XMLElementType(xml_element) => xml_element,
            _ => unreachable!(),
        };
        let mut trx = doc.transact_mut();
        let len = xml_element.len(&trx);
        if len >= 1 {
            let index = random_pick_num(len - 1, &params.insert_pos);
            xml_element.remove_range(&mut trx, index, 1).unwrap();
        }
    };

    let clear_op = |doc: &yrs::Doc, nest_input: &YrsNestType, _params: CRDTParam| {
        let xml_element = match nest_input {
            YrsNestType::XMLElementType(xml_element) => xml_element,
            _ => unreachable!(),
        };
        let mut trx = doc.transact_mut();
        let len = xml_element.len(&trx);
        for _ in 0..len {
            xml_element.remove_range(&mut trx, 0, 1).unwrap();
        }
    };

    ops.insert(NestDataOpType::Insert, Box::new(insert_op));
    ops.insert(NestDataOpType::Delete, Box::new(remove_op));
    ops.insert(NestDataOpType::Clear, Box::new(clear_op));

    ops
}
