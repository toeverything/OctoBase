use assert_json_diff::{assert_json_matches_no_panic, CompareMode, Config, NumericMode};
use yrs::{types::ToJson, Map, ReadTxn};

fn get_yrs_struct(
    trx: yrs::TransactionMut,
    block_id: Option<&str>,
) -> Result<serde_json::Value, String> {
    let json = trx
        .get_map("space:blocks")
        .ok_or_else(|| "get_yrs_struct: blocks not found".to_string())
        .and_then(|b| {
            if let Some(block_id) = block_id {
                b.get(&trx, block_id)
                    .ok_or("get_yrs_struct: block not found".into())
                    .map(|b| b.to_json(&trx))
            } else {
                Ok(b.to_json(&trx))
            }
        })?;
    drop(trx);

    serde_json::to_value(json)
        .map_err(|e| format!("get_yrs_struct: serde_json::to_value failed: {}", e))
}

fn get_jwst_struct(
    ws: &mut jwst_core::Workspace,
    block_id: Option<&str>,
) -> Result<serde_json::Value, String> {
    match ws.get_blocks() {
        Ok(blocks) => {
            if let Some(block_id) = block_id {
                blocks
                    .get(block_id)
                    .ok_or("get_jwst_struct: block not found".into())
                    .and_then(|b| {
                        serde_json::to_value(&b).map_err(|e| {
                            format!("get_yrs_struct: serde_json::to_value failed: {}", e)
                        })
                    })
            } else {
                serde_json::to_value(&blocks)
                    .map_err(|e| format!("get_jwst_struct: serde_json::to_value failed: {}", e))
            }
        }
        Err(e) => Err(format!("get_jwst_struct: get_blocks failed: {}", e)),
    }
}

pub fn workspace_compare(
    yrs_trx: yrs::TransactionMut,
    jwst_ws: &mut jwst_core::Workspace,
    block_id: Option<&str>,
) -> String {
    match get_yrs_struct(yrs_trx, block_id) {
        Ok(yrs_value) => match get_jwst_struct(jwst_ws, block_id) {
            Ok(jwst_value) => {
                if let Err(error) = assert_json_matches_no_panic(
                    &yrs_value,
                    &jwst_value,
                    Config::new(CompareMode::Strict).numeric_mode(NumericMode::AssumeFloat),
                ) {
                    format!("workspace_compare: struct compare failed: {}", error)
                } else {
                    "workspace_compare: struct compare success".into()
                }
            }
            Err(e) => e,
        },
        Err(e) => e,
    }
}
