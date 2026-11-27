use serde_json::json;
use systemprompt_drive::upload::{PlannedPart, already_sent, plan_parts};
use systemprompt_models::documents::client::DriveUpload;

const MIB: u64 = 1024 * 1024;

fn upload(size: u64, part_size: u64, missing: &[u32]) -> DriveUpload {
    let part_count = u32::try_from(size.div_ceil(part_size)).unwrap();
    serde_json::from_value(json!({
        "upload_id": "0b1c2d3e-4f50-4617-8293-a4b5c6d7e8f9",
        "name": "big.pdf", "mime": "application/pdf", "size": size,
        "sha256": "00", "part_size": part_size, "part_count": part_count, "status": "open",
        "missing_parts": missing, "expires_at": "2026-11-26T00:00:00Z"
    }))
    .unwrap()
}

fn length(bytes: u64) -> usize {
    usize::try_from(bytes).unwrap()
}

#[test]
fn parts_cover_the_file_and_the_last_is_short() {
    let size = 20 * MIB + 5;
    let open = upload(size, 8 * MIB, &[1, 2, 3]);
    let plan = plan_parts(&open, length(size)).unwrap();
    let eight = length(8 * MIB);
    assert_eq!(plan.len(), 3);
    assert_eq!(
        plan[0],
        PlannedPart {
            number: 1,
            range: 0..eight
        }
    );
    assert_eq!(plan[1].range, eight..2 * eight);
    assert_eq!(plan[2].range, 2 * eight..length(size));
    assert_eq!(already_sent(&open, &plan), 0);
}

#[test]
fn a_resumed_upload_sends_only_what_is_missing() {
    let size = 3 * 8 * MIB;
    let resumed = upload(size, 8 * MIB, &[3, 2, 2]);
    let plan = plan_parts(&resumed, length(size)).unwrap();
    assert_eq!(
        plan.iter().map(|part| part.number).collect::<Vec<_>>(),
        vec![2, 3]
    );
    assert_eq!(already_sent(&resumed, &plan), 8 * MIB);
}

#[test]
fn plans_refuse_other_bytes_and_foreign_parts() {
    let open = upload(10, 4, &[1, 2, 3]);
    assert!(plan_parts(&open, 9).is_err());
    assert!(plan_parts(&upload(10, 4, &[0]), 10).is_err());
    assert!(plan_parts(&upload(10, 4, &[4]), 10).is_err());
    let mut zero = upload(10, 4, &[1]);
    zero.part_size = 0;
    assert!(plan_parts(&zero, 10).is_err());
}
