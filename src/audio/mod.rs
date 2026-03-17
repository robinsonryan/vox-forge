pub mod capture;
pub mod vad;

/// Convert an amplitude value to decibels. Returns -100.0 for zero/silence.
pub fn amplitude_to_db(amplitude: f32) -> f32 {
    if amplitude <= 0.0 {
        -100.0
    } else {
        20.0 * amplitude.log10()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn amplitude_to_db_zero_returns_floor() {
        assert_eq!(amplitude_to_db(0.0), -100.0);
    }

    #[test]
    fn amplitude_to_db_negative_returns_floor() {
        assert_eq!(amplitude_to_db(-0.5), -100.0);
    }

    #[test]
    fn amplitude_to_db_unity_returns_zero() {
        assert!((amplitude_to_db(1.0) - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn amplitude_to_db_tenth_returns_minus_twenty() {
        assert!((amplitude_to_db(0.1) - (-20.0)).abs() < 0.01);
    }
}
