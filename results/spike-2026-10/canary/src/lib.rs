/// Returns the sum of the integers from 1 to `n`, inclusive.
pub fn sum_to(n: u64) -> u64 {
    (1..n).sum()
}

#[cfg(test)]
mod tests {
    use super::sum_to;

    #[test]
    fn sums_inclusive_range() {
        assert_eq!(sum_to(4), 10);
    }
}
