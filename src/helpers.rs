use std::cmp::{Ord, Ordering};

/// Some values can go negative. We need to convert the unsigned 16-bit
/// value into a signed one. The indication you haven't done this is values
/// close to 2^16 in metrics, representing negative values.
pub fn signed(raw_value: i64) -> i64 {
    match raw_value.cmp(&0x7FFF) {
        Ordering::Less | Ordering::Equal => raw_value,
        Ordering::Greater => raw_value - 0xFFFF,
    }
}

pub fn slug_name(name: &str) -> String {
    name.trim().to_lowercase().replace([' ', '-'], "_")
}
