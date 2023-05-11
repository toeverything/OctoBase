mod buffer;
mod integer;
mod string;

pub use buffer::{read_var_buffer, write_var_buffer};
pub use integer::{read_var_i64, read_var_u64, write_var_i64, write_var_u64};
pub use string::{read_var_string, write_var_string};

use super::*;
