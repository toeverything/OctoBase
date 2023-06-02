use super::*;

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
        xml_element
            .insert(&mut trx, index, XmlTextPrelim::new(params.value))
            .unwrap();
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic() {
        assert_eq!(gen_xml_element_ref_ops().len(), 3);
    }
}
