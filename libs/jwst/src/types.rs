use serde::Deserialize;
use utoipa::ToSchema;

pub struct BlockField {
    r#type: String,
    default: String,
}

#[derive(Default, Deserialize, ToSchema)]
#[schema(example = json!({"block_id": "jwstRf4rMzua7E", "pos": 0}))]
pub struct InsertChildren {
    pub(crate) block_id: String,
    pub(crate) pos: Option<u32>,
    pub(crate) before: Option<String>,
    pub(crate) after: Option<String>,
}

#[derive(Deserialize, ToSchema)]
#[schema(example = json!({"block_id": "jwstRf4rMzua7E"}))]
pub struct RemoveChildren {
    pub(crate) block_id: String,
}
