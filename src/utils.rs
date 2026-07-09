pub fn random_string(length: usize) -> String {
    use rand::RngExt;

    let mut rng = rand::rng();
    (&mut rng)
        .sample_iter(rand::distr::Alphanumeric)
        .take(length)
        .map(char::from)
        .collect()
}
