use super::*;

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
        xml_fragment
            .insert(&mut trx, index, XmlTextPrelim::new(params.value))
            .unwrap();
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
