use systemprompt_database::DrainNext;

#[derive(Debug)]
struct ClaimFailed;

#[test]
fn a_round_that_fails_outright_backs_off() {
    assert_eq!(
        DrainNext::of::<ClaimFailed>(&Err(ClaimFailed)),
        DrainNext::BackOff
    );
}

#[test]
fn a_cut_short_round_backs_off_whether_or_not_it_worked() {
    assert_eq!(DrainNext::after(true, true), DrainNext::BackOff);
    assert_eq!(DrainNext::after(false, true), DrainNext::BackOff);
}

#[test]
fn a_round_that_worked_drains_again_and_an_empty_one_listens() {
    assert_eq!(DrainNext::after(true, false), DrainNext::Drain);
    assert_eq!(DrainNext::after(false, false), DrainNext::Listen);
    assert_eq!(
        DrainNext::of::<ClaimFailed>(&Ok(DrainNext::Listen)),
        DrainNext::Listen
    );
}
