This reduced state-machine fixture preserves the bug fixed in Tokio commit
`9fccf5339d41c1f2f863f97b9133bc8a5a10bc28`. When a receiver is closed but a
send permit remains outstanding, `try_recv` must return `Empty`, because a
message can still arrive. It returns `Disconnected` only after all permits are
released and the queue is empty.
