use std::{
    fs::{read, write},
    io::{Error, ErrorKind},
    path::PathBuf,
};

use clap::Parser;
use jwst_codec::Doc;
use yrs::{updates::decoder::Decode, ReadTxn, StateVector, Transact, Update};

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
    jwst_merge(&args);
    yrs_merge(&args);
}

fn jwst_merge(args: &Args) {
    let updates = load_path(&args.path).unwrap();

    let mut doc = Doc::default();
    for (i, update) in updates.iter().enumerate() {
        println!("apply update{i} {} bytes", update.len());
        doc.apply_update_from_binary(update.clone()).unwrap();
    }
    let binary = doc.encode_update_v1().unwrap();
    println!("merged {} bytes", binary.len());
    write(
        args.output.clone().unwrap_or_else(|| format!("{}.jwst", args.path)),
        binary,
    )
    .unwrap();
}

fn yrs_merge(args: &Args) {
    let updates = load_path(&args.path).unwrap();

    let doc = yrs::Doc::new();
    for (i, update) in updates.iter().enumerate() {
        println!("apply update{i} {} bytes", update.len());
        doc.transact_mut().apply_update(Update::decode_v1(update).unwrap())
    }
    let binary = doc
        .transact()
        .encode_state_as_update_v1(&StateVector::default())
        .unwrap();
    println!("merged {} bytes", binary.len());
    write(
        args.output.clone().unwrap_or_else(|| format!("{}.yrs", args.path)),
        binary,
    )
    .unwrap();
}
