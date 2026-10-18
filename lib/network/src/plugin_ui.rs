//! Plugin windows and overhead notifications (rust-ro `docs/plugin-ui.md`). The map server sends them only after
//! [`build_client_features`] announced that this client draws them; the packets are framed and decoded by the
//! `plugin-ui` crate the server uses too.

use plugin_ui::wire::{self, EventMessage, FrameLength, ServerMessage};
use ragnarok_game::event::GameEvent;

/// Wire length of a plugin UI packet from the server: `Some(None)` when it is variable and the u16 after the id holds it.
pub fn frame_len(id: u16) -> Option<Option<usize>> {
    wire::server_frame_length(id).map(|length| match length {
        FrameLength::Fixed(length) => Some(length),
        FrameLength::Variable => None,
    })
}

/// The event for a packet framed by [`frame_len`]; a body that does not decode is logged and dropped.
pub fn dispatch(raw: &[u8]) -> Option<GameEvent> {
    match wire::decode_server(raw) {
        Ok(ServerMessage::Open(open)) => Some(GameEvent::PluginWindowOpen {
            plugin: open.plugin,
            window: Box::new(open.window),
        }),
        Ok(ServerMessage::Close(close)) => Some(GameEvent::PluginWindowClose {
            plugin: close.plugin,
            id: close.id,
        }),
        Ok(ServerMessage::Overhead { unit_id, message }) => Some(GameEvent::PluginOverhead {
            unit_id,
            plugin: message.plugin,
            overhead: Box::new(message.overhead),
        }),
        Err(error) => {
            tracing::warn!("dropped a plugin UI packet: {error}");
            None
        }
    }
}

/// Sent after each map enter.
pub fn build_client_features() -> Vec<u8> {
    wire::encode_features(wire::FEATURE_PLUGIN_UI)
}

pub fn build_event_packet(plugin: &str, event: plugin_ui::UiEvent) -> Option<Vec<u8>> {
    wire::encode_event(&EventMessage {
        plugin: plugin.to_string(),
        event,
    })
    .inspect_err(|error| tracing::warn!("plugin UI event not sent: {error}"))
    .ok()
}

#[cfg(test)]
mod tests {
    use plugin_ui::wire::{ClientMessage, OverheadMessage, WindowClose, WindowOpen};
    use plugin_ui::{Node, Overhead, UiEvent, Window};

    use super::*;

    #[test]
    fn server_packets_become_events() {
        let window = Window::new("shop", "Shop", 200, Node::text("Hello"));
        let open = wire::encode_open(&WindowOpen {
            plugin: "ui_shop".into(),
            window: window.clone(),
        })
        .unwrap();
        assert_eq!(
            frame_len(u16::from_le_bytes([open[0], open[1]])),
            Some(None)
        );
        match dispatch(&open) {
            Some(GameEvent::PluginWindowOpen {
                plugin,
                window: received,
            }) => {
                assert_eq!(plugin, "ui_shop");
                assert_eq!(*received, window);
            }
            other => panic!("{other:?}"),
        }
        let close = wire::encode_close(&WindowClose {
            plugin: "ui_shop".into(),
            id: "shop".into(),
        })
        .unwrap();
        assert!(
            matches!(dispatch(&close), Some(GameEvent::PluginWindowClose { id, .. }) if id == "shop")
        );
        let overhead = wire::encode_overhead(
            7,
            &OverheadMessage {
                plugin: "ui_shop".into(),
                overhead: Overhead::text("Hi", None, 500),
            },
        )
        .unwrap();
        assert!(matches!(
            dispatch(&overhead),
            Some(GameEvent::PluginOverhead { unit_id: 7, .. })
        ));
        let mut broken = open.clone();
        broken.truncate(8);
        broken[2..4].copy_from_slice(&8u16.to_le_bytes());
        assert!(dispatch(&broken).is_none());
        assert_eq!(
            frame_len(0xC3E6),
            None,
            "the client's own packets are not framed on the way in"
        );
    }

    #[test]
    fn client_packets_decode_on_the_server_side() {
        assert_eq!(
            wire::decode_client(&build_client_features()),
            Ok(ClientMessage::Features(wire::FEATURE_PLUGIN_UI))
        );
        let packet = build_event_packet("ui_shop", UiEvent::click("shop", "buy:501")).unwrap();
        match wire::decode_client(&packet) {
            Ok(ClientMessage::Event(message)) => assert_eq!(
                (message.plugin.as_str(), message.event.widget.as_str()),
                ("ui_shop", "buy:501")
            ),
            other => panic!("{other:?}"),
        }
    }
}
