pub fn expovariant_rand() -> f32 {
    let range = Range::new(0.0, 1.0);
    let mut rng = rand::thread_rng();
    let offset = range.ind_sample(&mut rng);
    -math::log(1.0 - offset)
}

