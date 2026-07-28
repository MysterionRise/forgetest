use ft_event_api::{message_text, Event, Severity};

#[test]
fn message_uses_struct_variant_and_severity() {
    let event = Event::Message {
        text: "disk almost full".into(),
        severity: Severity::Warning,
    };
    assert_eq!(message_text(&event), Some("disk almost full"));
    assert_eq!(
        event,
        Event::Message {
            text: "disk almost full".into(),
            severity: Severity::Warning,
        }
    );
}
