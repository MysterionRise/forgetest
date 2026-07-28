#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Message(String),
    Completed,
}

pub fn message_text(event: &Event) -> Option<&str> {
    match event {
        Event::Message(text) => Some(text),
        Event::Completed => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completed_has_no_message() {
        assert_eq!(message_text(&Event::Completed), None);
    }
}
