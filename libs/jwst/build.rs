use anyhow::Result;
use vergen::{vergen, Config};

fn main() -> Result<()> {
    vergen(Config::default())
}
