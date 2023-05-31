#![no_main]

#[cfg(fuzzing)]
use jwst_codec::{
    gen_nest_type_from_nest_type, gen_nest_type_from_root, CRDTParam, ManipulateSource, OpType,
    YrsMapOps, YrsNestType,
};
#[cfg(fuzzing)]
use libfuzzer_sys::fuzz_target;
#[cfg(fuzzing)]
use yrs::Transact;

#[cfg(fuzzing)]
fuzz_target!(|crdt_params: Vec<CRDTParam>| {
    let mut doc = yrs::Doc::new();
    let mut cur_crdt_nest_type: Option<YrsNestType> = None;
    let map_ops = YrsMapOps::new();
    for crdt_param in crdt_params {
        match crdt_param.op_type {
            OpType::HandleCurrent => match cur_crdt_nest_type.clone() {
                Some(YrsNestType::MapType(mut map_ref)) => {
                    map_ops.operate_map_ref(&doc, &mut map_ref, crdt_param.clone());
                }
                Some(..) => {}
                None => {}
            },
            OpType::CreateCRDTNestType => {
                cur_crdt_nest_type = match cur_crdt_nest_type {
                    None => gen_nest_type_from_root(&mut doc, &crdt_param),
                    Some(mut nest_type) => match crdt_param.manipulate_source {
                        ManipulateSource::CurrentNestType => Some(nest_type),
                        ManipulateSource::NewNestTypeFromYDocRoot => {
                            gen_nest_type_from_root(&mut doc, &crdt_param)
                        }
                        ManipulateSource::NewNestTypeFromCurrent => gen_nest_type_from_nest_type(
                            &mut doc,
                            crdt_param.clone(),
                            &mut nest_type,
                        ),
                    },
                };
            }
        };
    }

    let trx = doc.transact_mut();
    let binary_from_yrs = trx.encode_update_v1().unwrap();
    let doc = jwst_codec::Doc::new_from_binary(binary_from_yrs.clone()).unwrap();
    let binary = doc.encode_update_v1().unwrap();
    assert_eq!(binary, binary_from_yrs);
});
