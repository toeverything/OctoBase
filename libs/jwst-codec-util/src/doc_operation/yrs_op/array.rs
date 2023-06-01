use super::*;

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
            let str = text.get_string(&trx);
            let len = str.chars().fold(0, |acc, _| acc + 1);
            let index = random_pick_num(len, &insert_pos) as usize;
            let byte_start_offset = str
                .chars()
                .take(index)
                .fold(0, |acc, ch| acc + ch.len_utf8());

            Some(
                text.insert_embed(&mut trx, byte_start_offset as u32, array_prelim)
                    .unwrap(),
            )
        }
        YrsNestType::XMLTextType(xml_text) => {
            let str = xml_text.get_string(&trx);
            let len = str.chars().fold(0, |acc, _| acc + 1);
            let index = random_pick_num(len, &insert_pos) as usize;
            let byte_start_offset = str
                .chars()
                .take(index)
                .fold(0, |acc, ch| acc + ch.len_utf8());

            Some(
                xml_text
                    .insert_embed(&mut trx, byte_start_offset as u32, array_prelim)
                    .unwrap(),
            )
        }
        _ => None,
    }
}
