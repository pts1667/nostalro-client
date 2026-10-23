use std::io;
use std::panic;

use packets::packets::Packet;
use packets::packets_parser;
use ragnarok_profiling::debug::{self, PacketTrace};

use crate::transport::Transport;

const ZC_AID_PACKET_ID: u16 = 0x0283;

/// One framed packet off the wire.
pub enum Frame {
    Game(Box<dyn Packet>),
    /// A packet of rust-ro that the generated parser does not know, see
    /// [`crate::account`] and [`crate::plugin_ui`]; the raw bytes, header included.
    Custom(Vec<u8>),
}

#[derive(Debug)]
pub enum ConnectionError {
    Io(io::Error),
    Disconnected,
}

impl From<io::Error> for ConnectionError {
    fn from(err: io::Error) -> Self {
        ConnectionError::Io(err)
    }
}

/// Periodic keepalive/time packets excluded from packet tracing regardless of mode.
fn is_muted_packet(name: &str) -> bool {
    matches!(
        name,
        "PacketCzRequestTime" | "PacketCzPing" | "PacketZcNotifyTime"
    )
}

/// Packets whose payload carries a password: traced by name and length only.
fn carries_secret(name: &str) -> bool {
    matches!(name, "PacketCaLogin" | "PacketCzAckStorePassword")
}

pub struct Connection {
    transport: Transport,
    recv_buffer: Vec<u8>,
    expect_aid_preamble: bool,
}

impl Connection {
    /// `addr` is the server's `host:port`.
    pub async fn connect(addr: &str, expect_aid_preamble: bool) -> io::Result<Self> {
        Ok(Self {
            transport: Transport::connect(addr).await?,
            recv_buffer: Vec::with_capacity(4096),
            expect_aid_preamble,
        })
    }

    pub async fn send_packet(&mut self, data: &[u8], packetver: u32) -> io::Result<()> {
        if debug::packet_trace() == PacketTrace::All {
            let name =
                panic::catch_unwind(|| packets_parser::parse(data, packetver).name().to_string())
                    .unwrap_or_else(|_| "<parse panic>".to_string());
            if !is_muted_packet(&name) {
                if carries_secret(&name) {
                    tracing::info!("send packet: {} ({} bytes)", name, data.len());
                } else {
                    let preview: Vec<String> =
                        data.iter().take(16).map(|b| format!("{b:02x}")).collect();
                    tracing::info!(
                        "send packet: {} ({} bytes) [{}]",
                        name,
                        data.len(),
                        preview.join(" ")
                    );
                }
            }
        }
        self.transport.write_all(data).await
    }

    fn estimate_packet_len(data: &[u8]) -> usize {
        if data.len() >= 4 {
            let len = u16::from_le_bytes([data[2], data[3]]) as usize;
            if len >= 4 && len <= data.len() {
                return len;
            }
        }
        4.min(data.len())
    }

    pub async fn recv_packets(
        &mut self,
        packetver: u32,
    ) -> Result<Vec<Frame>, ConnectionError> {
        let start = self.recv_buffer.len();
        let n = self.transport.read_into(&mut self.recv_buffer).await?;
        if n == 0 {
            return Err(ConnectionError::Disconnected);
        }

        let trace = debug::packet_trace();

        if trace == PacketTrace::All {
            tracing::info!(
                "TCP read: {n} bytes, buffer_total={}, first_16={:02x?}",
                self.recv_buffer.len(),
                &self.recv_buffer[start..start + n.min(16)]
            );
        }

        if self.expect_aid_preamble {
            if self.recv_buffer.len() < 4 {
                return Ok(Vec::new());
            }
            if let Some(aid) = Self::take_bare_aid_preamble(&mut self.recv_buffer)
                && trace == PacketTrace::All
            {
                tracing::info!("consumed account_id preamble: {aid}");
            }
            self.expect_aid_preamble = false;
        }

        let packets = Self::drain_packets(&mut self.recv_buffer, packetver, trace);
        Ok(packets)
    }

    /// Drops the headerless account id a server may greet with. Zone servers from
    /// packetver 20070521 greet with `ZC_AID` instead, which the parser handles.
    fn take_bare_aid_preamble(buffer: &mut Vec<u8>) -> Option<u32> {
        if u16::from_le_bytes([buffer[0], buffer[1]]) == ZC_AID_PACKET_ID {
            return None;
        }
        let aid = u32::from_le_bytes([buffer[0], buffer[1], buffer[2], buffer[3]]);
        buffer.drain(..4);
        Some(aid)
    }

    /// Total wire length of the packet at the head of `data`.
    fn frame_len(data: &[u8], packetver: u32) -> FrameLen {
        if data.len() < 2 {
            return FrameLen::Incomplete;
        }
        let id = [data[0], data[1]];
        if let Some(len) = crate::account::frame_len(u16::from_le_bytes(id)) {
            return FrameLen::Known(len);
        }
        match crate::plugin_ui::frame_len(u16::from_le_bytes(id)) {
            Some(Some(len)) => return FrameLen::Known(len),
            Some(None) => return Self::declared_len(data),
            None => {}
        }
        if packets_parser::is_variable_length(id, packetver) {
            return Self::declared_len(data);
        }
        match packets_parser::packet_len(id, packetver) {
            Some(len) if len >= 2 => FrameLen::Known(len),
            _ => FrameLen::Unusable,
        }
    }

    /// The length a variable packet carries after its id.
    fn declared_len(data: &[u8]) -> FrameLen {
        if data.len() < 4 {
            return FrameLen::Incomplete;
        }
        let declared = u16::from_le_bytes([data[2], data[3]]) as usize;
        if declared < 4 {
            return FrameLen::Unusable;
        }
        FrameLen::Known(declared)
    }

    /// A rust-ro packet the generated parser does not know, handed on raw.
    fn is_custom(id: u16) -> bool {
        crate::account::frame_len(id).is_some() || crate::plugin_ui::frame_len(id).is_some()
    }

    fn drain_packets(
        buffer: &mut Vec<u8>,
        packetver: u32,
        trace: PacketTrace,
    ) -> Vec<Frame> {
        let mut packets = Vec::new();
        let mut offset = 0;

        while offset < buffer.len() {
            let remaining = &buffer[offset..];
            let expected = match Self::frame_len(remaining, packetver) {
                FrameLen::Known(len) => len,
                FrameLen::Incomplete => {
                    if trace == PacketTrace::All {
                        tracing::info!("partial header ({} bytes), awaiting more", remaining.len());
                    }
                    break;
                }
                FrameLen::Unusable => {
                    let skip = Self::estimate_packet_len(remaining);
                    tracing::info!(
                        "skipping unknown packet 0x{:02x}{:02x} ({skip} bytes), buffer_remaining={}",
                        remaining[0],
                        remaining.get(1).copied().unwrap_or(0),
                        remaining.len()
                    );
                    if trace == PacketTrace::All {
                        tracing::debug!("unknown packet dump: {:02x?}", &remaining[..skip]);
                    }
                    offset += skip;
                    continue;
                }
            };
            if expected > remaining.len() {
                if trace == PacketTrace::All {
                    tracing::info!(
                        "packet 0x{:02x}{:02x} needs {expected} bytes, have {} — awaiting more",
                        remaining[0],
                        remaining[1],
                        remaining.len()
                    );
                }
                break;
            }

            let parse_buf = &remaining[..expected];
            if Self::is_custom(u16::from_le_bytes([parse_buf[0], parse_buf[1]])) {
                packets.push(Frame::Custom(parse_buf.to_vec()));
                offset += expected;
                continue;
            }
            match panic::catch_unwind(|| packets_parser::parse(parse_buf, packetver)) {
                Ok(packet) => {
                    if packet.name() == "Unknown" {
                        tracing::info!(
                            "skipping unparsed packet 0x{:02x}{:02x} ({expected} bytes)",
                            parse_buf[0],
                            parse_buf[1]
                        );
                    } else {
                        if trace == PacketTrace::All && !is_muted_packet(packet.name()) {
                            tracing::info!(
                                "recv {} ({expected} bytes, remaining={})",
                                packet.name(),
                                remaining.len()
                            );
                        }
                        packets.push(Frame::Game(packet));
                    }
                }
                Err(_) => {
                    tracing::warn!(
                        "packet parse panic at offset {offset}, packet 0x{:02x}{:02x}, len {expected}",
                        parse_buf[0],
                        parse_buf[1]
                    );
                }
            }
            offset += expected;
        }

        buffer.drain(..offset);
        packets
    }
}

enum FrameLen {
    Known(usize),
    /// Too few bytes to read the length: wait for the next read.
    Incomplete,
    /// No length to work from — unknown id, or a declared length below the header.
    Unusable,
}

#[cfg(test)]
mod tests {
    use super::*;
    use packets::packets::{
        PacketHcNotifyZonesvr, PacketZcAcceptEnter2, PacketZcAid, PacketZcNotifyPlayerchat,
        PacketZcNotifyTime, PacketZcReqWearEquipAck,
    };
    use std::net::Ipv4Addr;

    fn games(frames: Vec<Frame>) -> Vec<Box<dyn Packet>> {
        frames
            .into_iter()
            .map(|frame| match frame {
                Frame::Game(packet) => packet,
                Frame::Custom(raw) => panic!("unexpected custom frame {raw:02x?}"),
            })
            .collect()
    }

    fn greeting_stream(packetver: u32, aid_bytes: &[u8]) -> Vec<u8> {
        let mut enter = PacketZcAcceptEnter2::new(packetver);
        enter.set_x_size(5);
        enter.set_y_size(5);
        enter.fill_raw_with_packetver(Some(packetver));
        let mut stream = aid_bytes.to_vec();
        stream.extend_from_slice(&enter.raw);
        stream
    }

    #[test]
    fn zone_greeting_frames_whatever_form_the_aid_takes() {
        let packetver = 20111102;
        let mut aid = PacketZcAid::new(packetver);
        aid.set_aid(2000000);
        aid.fill_raw_with_packetver(Some(packetver));

        let mut buffer = greeting_stream(packetver, &aid.raw);
        assert!(Connection::take_bare_aid_preamble(&mut buffer).is_none());
        let framed = games(Connection::drain_packets(&mut buffer, packetver, PacketTrace::None));
        assert_eq!(framed.len(), 2);
        assert_eq!(
            framed[0]
                .as_any()
                .downcast_ref::<PacketZcAid>()
                .unwrap()
                .aid,
            2000000
        );
        assert!(framed[1].as_any().is::<PacketZcAcceptEnter2>());
        assert!(buffer.is_empty());

        let mut buffer = greeting_stream(packetver, &2000000u32.to_le_bytes());
        assert_eq!(
            Connection::take_bare_aid_preamble(&mut buffer),
            Some(2000000)
        );
        let framed = games(Connection::drain_packets(&mut buffer, packetver, PacketTrace::None));
        assert_eq!(framed.len(), 1);
        assert!(framed[0].as_any().is::<PacketZcAcceptEnter2>());
        assert!(buffer.is_empty());
    }

    /// A packet arriving in two TCP reads must be held until complete, then
    /// parsed once — whether its length comes from a lookup or from its header.
    #[test]
    fn split_packets_are_held_until_complete() {
        let packetver = 20111102;

        let mut fixed = PacketZcNotifyTime::new(packetver);
        fixed.set_time(1234);
        fixed.fill_raw_with_packetver(Some(packetver));

        let mut variable = PacketZcNotifyPlayerchat::new(packetver);
        variable.set_msg("hello\0".to_string());
        variable.set_packet_length((PacketZcNotifyPlayerchat::base_len(packetver) + 6) as i16);
        variable.fill_raw_with_packetver(Some(packetver));

        for stream in [fixed.raw.clone(), variable.raw.clone()] {
            for split in 1..stream.len() {
                let mut buffer = stream[..split].to_vec();
                let framed = games(Connection::drain_packets(&mut buffer, packetver, PacketTrace::None));
                assert!(
                    framed.is_empty(),
                    "parsed a packet from {split} of {} bytes",
                    stream.len()
                );
                assert_eq!(buffer.len(), split, "dropped bytes of an incomplete packet");

                buffer.extend_from_slice(&stream[split..]);
                let framed = games(Connection::drain_packets(&mut buffer, packetver, PacketTrace::None));
                assert_eq!(framed.len(), 1, "split at {split} did not reassemble");
                assert_eq!(framed[0].raw(), &stream);
                assert!(buffer.is_empty());
            }
        }
    }

    /// The length framing advances by must equal the length parsing consumes,
    /// including for packets whose layout is packetver-gated.
    #[test]
    fn frame_len_matches_what_parsing_consumes() {
        for packetver in [20101122, 20101123, 20120307] {
            let mut ack = PacketZcReqWearEquipAck::new(packetver);
            ack.set_index(3);
            ack.fill_raw_with_packetver(Some(packetver));

            let expected = match Connection::frame_len(&ack.raw, packetver) {
                FrameLen::Known(len) => len,
                _ => panic!("no framing length at {packetver}"),
            };
            assert_eq!(expected, ack.raw.len());
            assert_eq!(expected, PacketZcReqWearEquipAck::base_len(packetver));
        }
    }

    /// A packet ending in a nested struct is longer than the fields before it, and
    /// must be framed at its full length rather than parsed short.
    #[test]
    fn a_packet_ending_in_a_nested_struct_is_framed_whole() {
        let packetver = 20111102;
        let mut stream = vec![0x71, 0x00];
        stream.extend_from_slice(&150000u32.to_le_bytes());
        stream.extend_from_slice(b"new_1-1.gat\0\0\0\0\0");
        stream.extend_from_slice(&u32::from(Ipv4Addr::new(127, 0, 0, 1)).to_be_bytes());
        stream.extend_from_slice(&6121i16.to_le_bytes());

        let expected = match Connection::frame_len(&stream, packetver) {
            FrameLen::Known(len) => len,
            _ => panic!("no framing length"),
        };
        assert_eq!(expected, stream.len());

        let mut short = stream[..stream.len() - 6].to_vec();
        assert!(Connection::drain_packets(&mut short, packetver, PacketTrace::None).is_empty());

        let mut buffer = stream.clone();
        let framed = games(Connection::drain_packets(&mut buffer, packetver, PacketTrace::None));
        assert_eq!(framed.len(), 1);
        let zonesvr = framed[0]
            .as_any()
            .downcast_ref::<PacketHcNotifyZonesvr>()
            .unwrap();
        assert_eq!(zonesvr.gid, 150000);
        assert_eq!(zonesvr.addr.port, 6121);
        assert!(buffer.is_empty());
    }

    /// A packet split mid-stream must not stall the packets that precede it.
    #[test]
    fn a_trailing_partial_packet_does_not_hold_back_the_complete_ones() {
        let packetver = 20111102;
        let mut time = PacketZcNotifyTime::new(packetver);
        time.set_time(7);
        time.fill_raw_with_packetver(Some(packetver));

        let mut buffer = time.raw.clone();
        buffer.extend_from_slice(&time.raw[..3]);

        let framed = games(Connection::drain_packets(&mut buffer, packetver, PacketTrace::None));
        assert_eq!(framed.len(), 1);
        assert_eq!(buffer, time.raw[..3]);
    }

    /// The login server's own answers are framed at their fixed length and
    /// handed on raw, between the packets the parser knows.
    #[test]
    fn custom_login_answers_are_framed_raw() {
        let packetver = 20120307;
        let mut time = PacketZcNotifyTime::new(packetver);
        time.set_time(7);
        time.fill_raw_with_packetver(Some(packetver));
        let info = [0xE3, 0xC3, 1, 0, 0x33, 0x0A, 0x33, 0x01];
        let created = [0xE1, 0xC3, 0, 0x81, 0x84, 0x1E, 0x00];

        let mut buffer = info[..5].to_vec();
        assert!(Connection::drain_packets(&mut buffer, packetver, PacketTrace::None).is_empty());
        buffer.extend_from_slice(&info[5..]);
        buffer.extend_from_slice(&time.raw);
        buffer.extend_from_slice(&created);

        let framed = Connection::drain_packets(&mut buffer, packetver, PacketTrace::None);
        assert!(buffer.is_empty());
        match &framed[..] {
            [Frame::Custom(a), Frame::Game(b), Frame::Custom(c)] => {
                assert_eq!(a, &info);
                assert!(b.as_any().is::<PacketZcNotifyTime>());
                assert_eq!(c, &created);
            }
            _ => panic!("expected custom, game, custom frames"),
        }
    }

    #[test]
    fn plugin_windows_are_framed_by_their_declared_length() {
        use plugin_ui::wire::{self, WindowOpen};
        let window = plugin_ui::Window::new("shop", "Shop", 200, plugin_ui::Node::text("x".repeat(5000)));
        let open = wire::encode_open(&WindowOpen { plugin: "ui_shop".into(), window }).unwrap();
        assert!(open.len() > 4096, "spans several reads");
        let mut buffer = open[..3].to_vec();
        assert!(Connection::drain_packets(&mut buffer, 20120307, PacketTrace::None).is_empty());
        buffer.extend_from_slice(&open[3..4096]);
        assert!(Connection::drain_packets(&mut buffer, 20120307, PacketTrace::None).is_empty());
        buffer.extend_from_slice(&open[4096..]);
        match &Connection::drain_packets(&mut buffer, 20120307, PacketTrace::None)[..] {
            [Frame::Custom(raw)] => assert_eq!(raw, &open),
            _ => panic!("expected one custom frame"),
        }
        assert!(buffer.is_empty());
    }
}
