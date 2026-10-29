use std::time::Duration;

use super::pause;

#[test]
fn the_ceiling_starts_at_two_milliseconds_and_doubles_to_sixty_four() {
    let ceilings: Vec<Duration> = (0..8).map(|attempt| pause(attempt, u16::MAX)).collect();
    let expected: Vec<Duration> = [2, 4, 8, 16, 32, 64, 64, 64]
        .into_iter()
        .map(Duration::from_millis)
        .collect();
    assert_eq!(ceilings, expected);
}

#[test]
fn a_pause_is_the_drawn_share_of_its_ceiling() {
    assert_eq!(pause(3, 0), Duration::ZERO);
    assert_eq!(pause(3, u16::MAX / 2), Duration::from_micros(7_999));
}

#[test]
fn a_late_attempt_never_overflows_the_ceiling() {
    assert_eq!(pause(u32::MAX, u16::MAX), Duration::from_millis(64));
}
