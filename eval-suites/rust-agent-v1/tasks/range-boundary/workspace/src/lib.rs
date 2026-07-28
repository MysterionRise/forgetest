pub fn plan_chunks(total: usize, chunk_size: usize) -> Vec<(usize, usize)> {
    if total == 0 || chunk_size == 0 {
        return Vec::new();
    }
    let mut chunks = Vec::new();
    let mut start = 0;
    while start < total {
        let end = (start + chunk_size).min(total.saturating_sub(1));
        chunks.push((start, end));
        start += chunk_size;
    }
    chunks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_inputs_have_no_chunks() {
        assert!(plan_chunks(0, 4).is_empty());
        assert!(plan_chunks(4, 0).is_empty());
    }
}
