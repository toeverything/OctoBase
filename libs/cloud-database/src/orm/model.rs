// use super::*;
// use sqlx::{postgres::PgRow, FromRow, Result, Row};

// impl FromRow<'_, PgRow> for Member {
//     fn from_row(row: &PgRow) -> Result<Self> {
//         let id = row.try_get("id")?;
//         let accepted = row.try_get("accepted")?;
//         let type_ = row.try_get("type")?;
//         let created_at = row.try_get("created_at")?;

//         let user = if let Some(email) = row.try_get("user_email")? {
//             UserCred::UnRegistered { email }
//         } else {
//             let id = row.try_get("user_id")?;
//             let name = row.try_get("user_name")?;
//             let email = row.try_get("user_table_email")?;
//             let avatar_url = row.try_get("avatar_url")?;
//             let created_at = row.try_get("user_created_at")?;
//             UserCred::Registered(User {
//                 id,
//                 name,
//                 email,
//                 avatar_url,
//                 created_at,
//             })
//         };

//         Ok(Member {
//             id,
//             accepted,
//             user,
//             type_,
//             created_at,
//         })
//     }
// }
