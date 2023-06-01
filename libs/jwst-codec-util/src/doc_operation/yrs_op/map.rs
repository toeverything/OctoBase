use super::*;

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
            let str = text.get_string(&trx);
            let len = str.chars().fold(0, |acc, _| acc + 1);
            let index = random_pick_num(len, &insert_pos) as usize;
            let byte_start_offset = str
                .chars()
                .take(index)
                .fold(0, |acc, ch| acc + ch.len_utf8());

            Some(
                text.insert_embed(&mut trx, byte_start_offset as u32, map_prelim)
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
                    .insert_embed(&mut trx, byte_start_offset as u32, map_prelim)
                    .unwrap(),
            )
        }
        _ => None,
    }
}
