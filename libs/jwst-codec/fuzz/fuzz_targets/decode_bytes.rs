#![no_main]

use jwst_codec::{read_var_buffer, read_var_i64, read_var_string, read_var_u64};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: Vec<u8>| {
    read_var_i64(&data);
    read_var_u64(&data);
    read_var_buffer(&data);
    read_var_string(&data);
});
