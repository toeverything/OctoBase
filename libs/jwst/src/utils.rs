use std::ops::RangeInclusive;

use lib0::encoding::Write;
use yrs::updates::encoder::{Encoder, EncoderV1};

const MSG_SYNC: usize = 0;
const MSG_SYNC_UPDATE: usize = 2;

fn write_sync<E: Encoder>(encoder: &mut E) {
    encoder.write_var(MSG_SYNC);
}

pub fn encode_update(update: &[u8]) -> Vec<u8> {
    let mut encoder = EncoderV1::new();

    write_sync(&mut encoder);

    encoder.write_var(MSG_SYNC_UPDATE);
    encoder.write_buf(update);

    encoder.to_vec()
}

const MAX_JS_INT: i64 = 0x001F_FFFF_FFFF_FFFF;
// The smallest int in js number.
const MIN_JS_INT: i64 = -MAX_JS_INT;
pub const JS_INT_RANGE: RangeInclusive<i64> = MIN_JS_INT..=MAX_JS_INT;
