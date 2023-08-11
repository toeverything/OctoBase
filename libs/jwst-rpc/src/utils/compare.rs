use assert_json_diff::{assert_json_matches_no_panic, CompareMode, Config};
use yrs::{types::ToJson, updates::decoder::Decode, ReadTxn, Transact};

fn get_yrs_struct(ws: &jwst::Workspace) -> Result<serde_json::Value, String> {
    ws.retry_with_trx(
        |trx| {
            trx.trx
                .get_map("space:blocks")
                .ok_or_else(|| "get_yrs_struct: blocks not found".into())
                .map(|b| b.to_json(&trx.trx))
        },
        50,
    )
    .map_err(|e| format!("get_yrs_update: get yrs transaction failed: {}", e))?
    .and_then(|json| {
        serde_json::to_value(json)
            .map_err(|e| format!("get_yrs_struct: serde_json::to_value failed: {}", e))
    })
}

fn get_jwst_struct(ws: &jwst_core::Workspace) -> Result<serde_json::Value, String> {
    match ws.doc().encode_update_v1() {
        Ok(update) => match yrs::Update::decode_v1(&update) {
            Ok(decoded) => {
                let doc = yrs::Doc::new();
                doc.transact_mut().apply_update(decoded);
                let trx = doc.transact();
                trx.get_map("space:blocks")
                    .ok_or_else(|| "get_yrs_struct: blocks not found".into())
                    .map(|b| b.to_json(&trx))
                    .and_then(|json| {
                        serde_json::to_value(json).map_err(|e| {
                            format!("get_yrs_struct: serde_json::to_value failed: {}", e)
                        })
                    })
            }
            Err(e) => Err(format!("get_jwst_update: decode update failed: {}", e)),
        },
        Err(e) => Err(format!("get_jwst_update: encode doc failed: {}", e)),
    }
}

pub fn workspace_compare(yrs_ws: &jwst::Workspace, jwst_ws: &jwst_core::Workspace) -> String {
    match get_yrs_struct(yrs_ws) {
        Ok(yrs_value) => match get_jwst_struct(jwst_ws) {
            Ok(jwst_value) => {
                if let Err(error) = assert_json_matches_no_panic(
                    &yrs_value,
                    &jwst_value,
                    Config::new(CompareMode::Strict),
                ) {
                    format!("workspace_compare: struct compare failed: {}", error)
                } else {
                    format!("workspace_compare: struct compare success")
                }
            }
            Err(e) => e,
        },
        Err(e) => e,
    }
}
