# How to Make Tests for OctoBase

OctoBase contains unit testing, fuzz testing, and integration testing. This document will introduce the testing methods and writing standards for each type of test.

## Unit Testing

Unit testing is the most basic testing method used to test the correctness of the code logic.

To write unit tests for OctoBase, you should write the tests near the functions that need to be tested, usually in the same `.rs` file. Place the test at the end of the code, wrapped in `mod tests`. Here’s an example:

```rust
fn some_function() {
    // code here
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_some_function() {
        // test here
    }
}
```

The test function should be named with the prefix `test_`, and it should be as simple as possible. If the test function is too complex, you should split it into multiple simpler test functions.

You can run these tests using `cargo test`, but it is recommended to use `cargo-nextest` for faster test runs and a more friendly error message. Here are the commands to install and run `cargo-nextest`:

```bash
# Install cargo-nextest
cargo install cargo-binstall
cargo binstall cargo-nextest --secure

# Run all tests
cargo nextest run
```

## Fuzz Testing

Fuzz testing is a testing method that uses random data to test the robustness of the program. This type of testing is usually used to test the correctness of the program when the input data is abnormal.

In OctoBase, we use `cargo-fuzz` and `libfuzzer_sys` for fuzz testing. Each crate that includes fuzz testing has a `fuzz` folder that contains .rs files for each fuzz test. These tests use random data to test the correctness of the program. Here’s an example of a `fuzz` test for OctoBase:

```rust
#![no_main]

use jwst_codec::{read_var_i64, write_var_i64};
use lib0::encoding::Write;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: Vec<i64>| {
    for i in data {
        if i == i64::MIN {
            continue;
        }

        let mut buf1 = Vec::new();
        write_var_i64(&mut buf1, i).unwrap();

        let mut buf2 = Vec::new();
        buf2.write_var(i);

        assert_eq!(read_var_i64(&buf1).unwrap().1, i);
        assert_eq!(read_var_i64(&buf2).unwrap().1, i);
    }
});
```

This fuzz test takes an random generated `i64` array as input and checks the correctness of the encoding and decoding operations using the `jwst-codec` and `lib0` libraries.

To run fuzz tests, you need to install `cargo-fuzz` first. Here are the commands to install and run fuzz tests:

```bash
# Install cargo-fuzz
cargo install cargo-fuzz
# Navigate to the directory of the crate that contains the fuzz tests
cd libs/some_lib
# List all fuzz tests in the current crate
cargo fuzz list
# Run specific fuzz test
cargo fuzz run fuzz_test_name
```

It is important to remember that `cargo-fuzz` can only test fuzz tests for the current folder's crate. Therefore, you should navigate to the directory of the crate that contains the fuzz tests to run the fuzz tests for that crate.

## Integration Testing

🚧 WIP
