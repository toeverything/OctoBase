use super::*;
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

        let str = text.get_string(&trx);
        let len = str.chars().fold(0, |acc, _| acc + 1);
        let index = random_pick_num(len, &params.insert_pos) as usize;
        let byte_start_offset = str
            .chars()
            .take(index)
            .fold(0, |acc, ch| acc + ch.len_utf8());

        text.insert(&mut trx, byte_start_offset as u32, &params.value)
            .unwrap();
    };

    let remove_op = |doc: &yrs::Doc, nest_input: &YrsNestType, params: CRDTParam| {
        let text = match nest_input {
            YrsNestType::TextType(text) => text,
            _ => unreachable!(),
        };
        let mut trx = doc.transact_mut();

        let str = text.get_string(&trx);
        let len = str.chars().fold(0, |acc, _| acc + 1);
        if len < 1 {
            return;
        }
        let index = random_pick_num(len - 1, &params.insert_pos) as usize;
        let byte_start_offset = str
            .chars()
            .take(index)
            .fold(0, |acc, ch| acc + ch.len_utf8());

        let char_byte_len = str.chars().skip(index).next().unwrap().len_utf8();
        text.remove_range(&mut trx, byte_start_offset as u32, char_byte_len as u32)
            .unwrap();
    };

    let clear_op = |doc: &yrs::Doc, nest_input: &YrsNestType, _params: CRDTParam| {
        let text = match nest_input {
            YrsNestType::TextType(text) => text,
            _ => unreachable!(),
        };
        let mut trx = doc.transact_mut();

        let str = text.get_string(&trx);
        let byte_len = str.chars().fold(0, |acc, ch| acc + ch.len_utf8());

        text.remove_range(&mut trx, 0, byte_len as u32).unwrap();
    };

    ops.insert(NestDataOpType::Insert, Box::new(insert_op));
    ops.insert(NestDataOpType::Delete, Box::new(remove_op));
    ops.insert(NestDataOpType::Clear, Box::new(clear_op));

    ops
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
            let str = text.get_string(&trx);
            let len = str.chars().fold(0, |acc, _| acc + 1);
            let index = random_pick_num(len, &insert_pos) as usize;
            let byte_start_offset = str
                .chars()
                .take(index)
                .fold(0, |acc, ch| acc + ch.len_utf8());

            Some(
                text.insert_embed(&mut trx, byte_start_offset as u32, text_prelim)
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
                    .insert_embed(&mut trx, byte_start_offset as u32, text_prelim)
                    .unwrap(),
            )
        }
        _ => None,
    }
}
