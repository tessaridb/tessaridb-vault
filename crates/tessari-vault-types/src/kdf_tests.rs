use super::{KdfParams, KdfParamsError};

#[test]
fn the_rfc_second_option_is_accepted() {
    assert!(KdfParams::new(65_536, 3, 4).is_ok());
}

#[test]
fn memory_below_the_floor_is_refused() {
    assert_eq!(
        KdfParams::new(19_456, 2, 1),
        Err(KdfParamsError::MemoryOutOfBounds(19_456))
    );
}

#[test]
fn zero_passes_and_too_many_lanes_are_refused() {
    assert_eq!(
        KdfParams::new(65_536, 0, 4),
        Err(KdfParamsError::PassesOutOfBounds(0))
    );
    assert_eq!(
        KdfParams::new(65_536, 3, 9),
        Err(KdfParamsError::LanesOutOfBounds(9))
    );
}

#[test]
fn memory_above_the_ceiling_is_refused() {
    assert!(KdfParams::new(4_194_304, 1, 1).is_err());
}
