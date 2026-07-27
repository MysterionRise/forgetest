use ft_mpsc_close_permit::{ChannelState, TryRecv};

#[test]
fn outstanding_permit_keeps_closed_channel_logically_open() {
    let mut channel = ChannelState::default();
    channel.reserve();
    channel.close();
    assert_eq!(channel.try_recv(), TryRecv::Empty);
    channel.release_permit();
    assert_eq!(channel.try_recv(), TryRecv::Disconnected);
}
