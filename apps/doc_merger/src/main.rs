use std::{
    fs::{read, write},
    io::{Error, ErrorKind},
    path::PathBuf,
};

use clap::Parser;
use jwst_codec::Doc;
use yrs::{types::ToJson, updates::decoder::Decode, ReadTxn, StateVector, Transact, Update};

/// ybinary merger
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path of the ybinary to read
    #[arg(short, long)]
    path: String,

    /// Output file
    #[arg(short, long)]
    output: Option<String>,
}

fn load_path(path: &str) -> Result<Vec<Vec<u8>>, Error> {
    let path = PathBuf::from(path);
    if path.is_dir() {
        let mut updates = Vec::new();
        let mut paths = path
            .read_dir()?
            .filter_map(|entry| {
                let entry = entry.ok()?;
                if entry.path().is_file() {
                    Some(entry.path())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        paths.sort();

        for path in paths {
            println!("read {:?}", path);
            updates.push(read(path)?);
        }
        Ok(updates)
    } else if path.is_file() {
        Ok(vec![read(path)?])
    } else {
        Err(Error::new(ErrorKind::NotFound, "not a file or directory"))
    }
}

fn main() {
    let args = Args::parse();
    jwst_merge(
        &args.path,
        &args.output.clone().unwrap_or_else(|| format!("{}.jwst", args.path)),
    );
    // std::io::stdin().read_line(&mut String::new()).unwrap();
    yrs_merge(
        &args.path,
        &args.output.clone().unwrap_or_else(|| format!("{}.yrs", args.path)),
    );
}

fn jwst_merge(path: &str, output: &str) {
    let updates = load_path(path).unwrap();

    let mut doc = Doc::default();
    for (i, update) in updates.iter().enumerate() {
        println!("apply update{i} {} bytes", update.len());
        doc.apply_update_from_binary(update.clone()).unwrap();
        println!("status: {:?}", doc.store_status());
    }
    doc.gc().unwrap();

    let (binary, json) = {
        let json = serde_json::to_string_pretty(&doc.get_map("space:blocks").unwrap()).unwrap();
        let binary = doc.encode_update_v1().unwrap();

        println!("merged {} bytes, json {} bytes", binary.len(), json.len());

        (binary, doc.get_map("space:blocks").unwrap())
    };

    {
        let mut doc = Doc::default();
        doc.apply_update_from_binary(binary.clone()).unwrap();
        let new_binary = doc.encode_update_v1().unwrap();
        let new_json = serde_json::to_string_pretty(&doc.get_map("space:blocks").unwrap()).unwrap();
        assert_json_diff::assert_json_eq!(doc.get_map("space:blocks").unwrap(), json);

        println!(
            "re-encoded {} bytes, new json {} bytes",
            new_binary.len(),
            new_json.len()
        );
    };
    write(output, binary).unwrap();
}

fn yrs_merge(path: &str, output: &str) {
    let updates = load_path(path).unwrap();

    let doc = yrs::Doc::new();
    for (i, update) in updates.iter().enumerate() {
        println!("apply update{i} {} bytes", update.len());
        doc.transact_mut().apply_update(Update::decode_v1(update).unwrap())
    }
    let binary = doc
        .transact()
        .encode_state_as_update_v1(&StateVector::default())
        .unwrap();
    let map = doc.get_or_insert_map("space:blocks");
    let json = serde_json::to_string_pretty(&map.to_json(&doc.transact())).unwrap();

    println!("merged {} bytes, json {} bytes", binary.len(), json.len());
    write(output, binary).unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "only for debug"]
    fn test_gc() {
        jwst_merge("/Users/ds/Downloads/out", "/Users/ds/Downloads/out.jwst");
        yrs_merge("/Users/ds/Downloads/out", "/Users/ds/Downloads/out.yrs");
    }
}
