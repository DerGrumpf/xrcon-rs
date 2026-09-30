use darkplaces::{KickTarget, kick_command};

#[test]
fn by_name() {
    assert_eq!(kick_command(KickTarget::Name(b"Phil"), None), b"kick Phil");
}

#[test]
fn by_slot_with_reason() {
    assert_eq!(
        kick_command(KickTarget::Slot(3), Some(b"spam")),
        b"kick # 3 spam"
    );
}
