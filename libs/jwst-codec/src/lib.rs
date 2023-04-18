mod codec;
mod doc;

pub use codec::{
    read_var_buffer, read_var_i64, read_var_string, read_var_u64, write_var_i64, write_var_u64,
};
pub use doc::{read_content, read_item, read_item_id, read_update, Content, Id, Item, Update};

use nom::IResult;

pub fn parse_doc_update(input: &[u8]) -> IResult<&[u8], Update> {
    let (input, update) = read_update(input)?;
    debug_assert_eq!(input.len(), 0);
    Ok((input, update))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;
    use std::{num::ParseIntError, path::PathBuf};

    #[test]
    fn test_parse_doc() {
        let docs = [
            (include_bytes!("./fixtures/basic.bin").to_vec(), 1, 188),
            (include_bytes!("./fixtures/database.bin").to_vec(), 1, 149),
            (include_bytes!("./fixtures/large.bin").to_vec(), 1, 9036),
        ];

        for (doc, clients, structs) in docs {
            let (tail, update) = parse_doc_update(&doc).unwrap();

            assert_eq!(tail.len(), 0);
            assert_eq!(update.structs.len(), clients);
            assert_eq!(
                update
                    .structs
                    .iter()
                    .map(|s| s.structs.len())
                    .sum::<usize>(),
                structs
            );
        }
    }

    fn decode_hex(s: &str) -> Result<Vec<u8>, ParseIntError> {
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16))
            .collect()
    }

    #[allow(dead_code)]
    #[derive(Deserialize, Debug)]
    struct Data {
        id: u64,
        workspace: String,
        timestamp: String,
        blob: String,
    }

    #[ignore = "just for local data test"]
    #[test]
    fn test_parse_local_doc() {
        let json =
            serde_json::from_slice::<Vec<Data>>(include_bytes!("./fixtures/local_docs.json"))
                .unwrap();

        for ws in json {
            let data = &ws.blob[5..=(ws.blob.len() - 2)];
            if let Ok(data) = decode_hex(data) {
                match parse_doc_update(&data) {
                    Ok((_tail, update)) => {
                        println!(
                            "workspace: {}, global structs: {}, total structs: {}",
                            ws.workspace,
                            update.structs.len(),
                            update
                                .structs
                                .iter()
                                .map(|s| s.structs.len())
                                .sum::<usize>()
                        );
                    }
                    Err(_e) => {
                        std::fs::write(
                            PathBuf::from("./src/fixtures/invalid")
                                .join(format!("{}.ydoc", ws.workspace)),
                            data,
                        )
                        .unwrap();
                        println!("doc error: {}", ws.workspace);
                    }
                }
            } else {
                println!("error origin data: {}", ws.workspace);
            }
        }
    }
}
