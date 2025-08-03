use std::cmp::{Ord, Ordering};

/// Some values can go negative. We need to convert the unsigned 16-bit
/// value into a signed one. The indication you haven't done this is values
/// close to 2^16 in metrics, representing negative values.
#[must_use] pub fn signed(raw_value: i64) -> i64 {
    match raw_value.cmp(&0x7FFF) {
        Ordering::Less | Ordering::Equal => raw_value,
        Ordering::Greater => raw_value - 0xFFFF,
    }
}

#[must_use] pub fn slug_name(name: &str) -> String {
    name.trim().to_lowercase().replace([' ', '-'], "_")
}

/// Given a list of registers, return a list containing the starting registers in a consective row,
/// and the number of consecutive registers.
/// eg [1, 2, 3, 5, 6, 9] -> [(1, 3), (5, 2), (9, 1)]
#[must_use] pub fn group_consecutive(mut registers: Vec<u16>) -> Vec<(u16, u16)> {
    let mut out: Vec<(u16, u16)> = Vec::new();
    let mut consecutive_number: u16 = 0;
    let mut starting_reg: u16 = 0;
    let mut prev_reg: Option<u16> = None;
    registers.sort_unstable();
    for reg in &registers {
        if let Some(p) = prev_reg {
            if (p + 1) == *reg {
                consecutive_number += 1;
            } else {
                out.push((starting_reg, consecutive_number));
                starting_reg = *reg;
                consecutive_number = 1;
            }
        } else {
            starting_reg = *reg;
            consecutive_number = 1;
        }
        prev_reg = Some(*reg);
    }
    out.push((starting_reg, consecutive_number));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_group_consecutive() {
        let input = vec![1, 2, 3, 5, 6, 9];
        let expected_out = [(1, 3), (5, 2), (9, 1)];
        let out = group_consecutive(input);
        assert_eq!(out, expected_out);
    }

    #[test]
    fn test_group_consecutive_unsorted() {
        let input = vec![3, 1, 6, 2, 9, 5];
        let expected_out = [(1, 3), (5, 2), (9, 1)];
        let out = group_consecutive(input);
        assert_eq!(out, expected_out);
    }
}
