pub fn normalized_total(mut values: Vec<i64>) -> i64 {
    values.sort_unstable();
    values.into_iter().sum()
}
