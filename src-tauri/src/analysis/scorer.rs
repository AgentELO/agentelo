pub fn calculate_elo(current_elo: f64, session_score: f64, k: f64) -> f64 {
    let actual = session_score / 100.0;
    let expected = 1.0 / (1.0 + 10.0_f64.powf((1200.0 - current_elo) / 400.0));
    let new_elo = current_elo + k * (actual - expected);
    (new_elo * 10.0).round() / 10.0
}
