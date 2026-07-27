use ft_range_boundary::plan_chunks;

#[test]
fn chunks_use_exclusive_end_offsets() {
    assert_eq!(plan_chunks(10, 4), vec![(0, 4), (4, 8), (8, 10)]);
    assert_eq!(plan_chunks(3, 8), vec![(0, 3)]);
}
