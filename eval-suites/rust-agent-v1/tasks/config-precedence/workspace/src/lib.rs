pub fn effective_timeout(
    cli: Option<u64>,
    explicit_file: Option<u64>,
    user: Option<u64>,
    default: u64,
) -> u64 {
    user.or(explicit_file).or(cli).unwrap_or(default)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn falls_back_to_default() {
        assert_eq!(effective_timeout(None, None, None, 30), 30);
    }

    #[test]
    fn uses_user_value_when_it_is_the_only_override() {
        assert_eq!(effective_timeout(None, None, Some(45), 30), 45);
    }
}
