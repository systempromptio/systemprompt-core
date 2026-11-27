use chrono::TimeDelta;
use systemprompt_device::protocol::receipts::build_receipt;
use systemprompt_identifiers::{
    ConsumerInstallationId, DeviceId, InstallationReceiptId, NativeSessionId,
};
use systemprompt_models::feedback::receipts::ConsumerReceiptRequest;

use systemprompt_harness::sync::outbox::{
    self, Binding, BindingAnswer, Delivery, OUTBOX_LIMIT, Outbox, ReceiptAnswer, SESSIONS_PER_ENTRY,
};

use crate::sync::fakes::{at, plan_for, publication, readback};

fn receipt(generation: i64) -> ConsumerReceiptRequest {
    let plan = plan_for(&publication(generation));
    build_receipt(
        &plan,
        &readback(&plan),
        ConsumerInstallationId::new("inst-1"),
        at(0),
    )
    .unwrap()
}

fn acknowledged(outbox: &mut Outbox, index: usize) {
    let answer = ReceiptAnswer::Acknowledged {
        receipt_id: InstallationReceiptId::new("r-1"),
        verified: true,
    };
    outbox.settle_receipt(index, answer, at(0));
}

#[test]
fn the_same_receipt_again_keeps_its_entry_and_state() {
    let mut outbox = Outbox::new(DeviceId::try_new("d-1").unwrap());
    outbox.enqueue(receipt(1), at(0));
    acknowledged(&mut outbox, 0);
    let mut again = receipt(1);
    again.observed_at = at(100);
    outbox.enqueue(again, at(100));
    assert_eq!(outbox.entries.len(), 1);
    assert_eq!(outbox.entries[0].delivery, Delivery::Acknowledged);
    assert!(outbox.holds(&publication(1)));
    assert!(!outbox.holds(&publication(2)));
}

#[test]
fn a_newer_generation_supersedes_the_older() {
    let mut outbox = Outbox::new(DeviceId::try_new("d-1").unwrap());
    outbox.enqueue(receipt(1), at(0));
    outbox.enqueue(receipt(2), at(0));
    assert_eq!(outbox.entries.len(), 1);
    assert_eq!(outbox.entries[0].receipt.generation, 2);
    assert_eq!(outbox.entries[0].delivery, Delivery::Pending);
}

#[test]
fn receipts_settle_as_acknowledged_refused_or_retried_with_backoff() {
    let mut outbox = Outbox::new(DeviceId::try_new("d-1").unwrap());
    outbox.enqueue(receipt(1), at(0));
    assert_eq!(outbox.due_receipts(at(0)), vec![0]);
    outbox.settle_receipt(0, ReceiptAnswer::Retry, at(0));
    assert_eq!(outbox.due_receipts(at(1)), Vec::<usize>::new());
    assert_eq!(outbox.due_receipts(at(2)), vec![0]);
    outbox.settle_receipt(0, ReceiptAnswer::Retry, at(2));
    assert_eq!(outbox.due_receipts(at(5)), Vec::<usize>::new());
    assert_eq!(outbox.due_receipts(at(6)), vec![0]);
    outbox.settle_receipt(0, ReceiptAnswer::Conflict, at(6));
    assert_eq!(outbox.due_receipts(at(10_000)), Vec::<usize>::new());
    assert!(!outbox.undelivered());
}

#[test]
fn sessions_bind_to_acknowledged_verified_receipts_once() {
    let mut outbox = Outbox::new(DeviceId::try_new("d-1").unwrap());
    outbox.enqueue(receipt(1), at(0));
    let session = NativeSessionId::new("585a6083-9a91-527d-9414-85ec68884f9a");
    outbox.bind_session(&session);
    assert_eq!(outbox.binding_receipt(0, at(0)), None);
    acknowledged(&mut outbox, 0);
    assert_eq!(
        outbox.binding_receipt(0, at(0)),
        Some(InstallationReceiptId::new("r-1"))
    );
    assert_eq!(outbox.pending_sessions(0), vec![session.clone()]);
    assert!(outbox.undelivered());
    outbox.settle_binding(0, &session, BindingAnswer::Bound, at(0));
    assert_eq!(outbox.pending_sessions(0), Vec::<NativeSessionId>::new());
    outbox.bind_session(&session);
    assert_eq!(
        outbox.entries[0].sessions.get(&session),
        Some(&Binding::Bound)
    );
    assert!(!outbox.undelivered());
}

#[test]
fn a_binding_that_fails_defers_its_receipt() {
    let mut outbox = Outbox::new(DeviceId::try_new("d-1").unwrap());
    outbox.enqueue(receipt(1), at(0));
    acknowledged(&mut outbox, 0);
    let session = NativeSessionId::new("s-1");
    outbox.bind_session(&session);
    outbox.settle_binding(0, &session, BindingAnswer::Retry, at(0));
    assert_eq!(outbox.binding_receipt(0, at(1)), None);
    assert!(outbox.binding_receipt(0, at(2)).is_some());
}

#[test]
fn at_most_256_sessions_wait_per_receipt() {
    let mut outbox = Outbox::new(DeviceId::try_new("d-1").unwrap());
    outbox.enqueue(receipt(1), at(0));
    for index in 0..SESSIONS_PER_ENTRY + 10 {
        outbox.bind_session(&NativeSessionId::new(format!("s-{index}")));
    }
    assert_eq!(outbox.entries[0].sessions.len(), SESSIONS_PER_ENTRY);
}

#[test]
fn a_full_outbox_drops_a_settled_entry_first() {
    let mut outbox = Outbox::new(DeviceId::try_new("d-1").unwrap());
    for index in 0..OUTBOX_LIMIT {
        let mut entry = receipt(1);
        entry.installation_id = ConsumerInstallationId::new(format!("inst-{index}"));
        outbox.enqueue(entry, at(0));
    }
    outbox.settle_receipt(3, ReceiptAnswer::Rejected, at(0));
    let mut extra = receipt(1);
    extra.installation_id = ConsumerInstallationId::new("inst-extra");
    outbox.enqueue(extra, at(0) + TimeDelta::seconds(1));
    assert_eq!(outbox.entries.len(), OUTBOX_LIMIT);
    assert!(
        outbox
            .entries
            .iter()
            .all(|entry| entry.delivery == Delivery::Pending)
    );
}

#[test]
fn the_file_round_trips_and_another_device_reads_as_empty() {
    let dir = tempfile::tempdir().unwrap();
    let path = outbox::outbox_path(dir.path(), "a.json");
    let mut held = Outbox::new(DeviceId::try_new("d-1").unwrap());
    held.enqueue(receipt(1), at(0));
    outbox::save(&path, &held).unwrap();
    assert_eq!(
        outbox::load(&path, &DeviceId::try_new("d-1").unwrap()),
        held
    );
    assert_eq!(
        outbox::load(&path, &DeviceId::try_new("d-2").unwrap()).entries,
        Vec::<outbox::Entry>::new()
    );
    std::fs::write(&path, b"not json").unwrap();
    assert_eq!(
        outbox::load(&path, &DeviceId::try_new("d-1").unwrap()).entries,
        Vec::<outbox::Entry>::new()
    );
}
