#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TryRecv {
    Item,
    Empty,
    Disconnected,
}

#[derive(Debug, Default)]
pub struct ChannelState {
    closed: bool,
    outstanding_permits: usize,
    queued: usize,
}

impl ChannelState {
    pub fn reserve(&mut self) {
        self.outstanding_permits += 1;
    }

    pub fn release_permit(&mut self) {
        self.outstanding_permits = self.outstanding_permits.saturating_sub(1);
    }

    pub fn close(&mut self) {
        self.closed = true;
    }

    pub fn try_recv(&mut self) -> TryRecv {
        if self.queued > 0 {
            self.queued -= 1;
            TryRecv::Item
        } else if self.closed {
            TryRecv::Disconnected
        } else {
            TryRecv::Empty
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closed_idle_channel_is_disconnected() {
        let mut channel = ChannelState::default();
        channel.close();
        assert_eq!(channel.try_recv(), TryRecv::Disconnected);
    }
}
