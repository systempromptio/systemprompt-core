use systemprompt_extension::contribute::ContributionSink;
use systemprompt_extension::{Contributions, Extension};
use systemprompt_identifiers::ExtensionId;
use systemprompt_notifications::NotificationsExtension;

#[test]
fn no_member_provides_the_notification_service_or_consumer_cell() {
    let mut sink = ContributionSink::default();
    NotificationsExtension
        .contribute(&mut Contributions::__new(
            ExtensionId::new("notifications"),
            &mut sink,
        ))
        .expect("the notifications member contributes");
    let provided: Vec<&str> = sink.provides().iter().map(|entry| entry.type_name()).collect();
    assert!(
        provided.iter().all(|name| !name.contains("NotificationsService")
            && !name.contains("NotificationsConsumer")
            && !name.contains("LateBound")),
        "the notifications member provides a handle cell: {provided:?}"
    );
}
