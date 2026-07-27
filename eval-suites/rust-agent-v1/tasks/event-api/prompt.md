Migrate `Event::Message(String)` to a struct variant with fields `text: String`
and `severity: Severity`. Add public `Severity::{Info, Warning, Error}` and
update `message_text`. Preserve `Event::Completed` and its existing behavior.
