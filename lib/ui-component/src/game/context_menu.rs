use crate::helper::window_chrome::text_color;
use ragnarok_game::event::{Describe, GameEvent};
use ragnarok_ui::draw::{self, DrawCall, TextureRef};
use ragnarok_ui::frame::{UiFrame, WidgetId};
use ragnarok_ui::rect::Rect;

const MENU_BASE_ID: u32 = 4700;
const ITEM_W: f32 = 120.0;
const ITEM_H: f32 = 18.0;

#[derive(Clone)]
pub enum ContextMenuAction {
    InviteToParty {
        target_aid: u32,
    },
    RequestTrade {
        target_aid: u32,
    },
    AdoptBaby {
        target_aid: u32,
    },
    Whisper {
        name: String,
    },
    ChangeGuildPosition {
        aid: u32,
        gid: u32,
        position_id: i32,
    },
    ExpelFromGuild {
        aid: u32,
        gid: u32,
        name: String,
    },
    GuildLeave,
    GuildInvite {
        target_char_id: u32,
    },
    GuildAlly {
        target_aid: u32,
    },
    GuildHostile {
        target_aid: u32,
    },
    CompanionShowInfo {
        is_mercenary: bool,
    },
    CompanionFeed,
    CompanionStandby {
        is_mercenary: bool,
    },
    CompanionPatrol {
        is_mercenary: bool,
    },
    CompanionAiConfig,
    KickFromChatRoom {
        name: String,
    },
    ChangeChatOwner {
        name: String,
    },
    PetShowInfo,
    PetFeed,
    PetCommand {
        csub: i8,
    },
    GiveMannerPoint {
        target_aid: u32,
        positive: bool,
    },
    AccountName {
        aid: u32,
    },
    /// Opens the database window at `path`.
    ShowInfo {
        path: String,
    },
    Describe(Describe),
}

/// The event a right-click on something with a database entry sends: a menu at
/// the pointer with "Info" for `info` and "Description" for `describe`.
pub fn info_menu(ui: &UiFrame, info: Option<String>, describe: Option<Describe>) -> GameEvent {
    GameEvent::OpenInfoMenu { x: ui.ctx.mouse_x, y: ui.ctx.mouse_y, info, describe }
}

/// The database path of an item.
pub fn item_info_path(item_id: u16) -> String {
    format!("/db/items/{item_id}")
}

pub struct ContextMenuItem {
    pub label: String,
    pub action: ContextMenuAction,
}

#[derive(Default)]
pub struct ContextMenu {
    open: bool,
    x: f32,
    y: f32,
    items: Vec<ContextMenuItem>,
}

impl ContextMenu {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn open_at(&mut self, x: f32, y: f32, items: Vec<ContextMenuItem>) {
        if items.is_empty() {
            return;
        }
        self.open = true;
        self.x = x;
        self.y = y;
        self.items = items;
    }

    pub fn close(&mut self) {
        self.open = false;
        self.items.clear();
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn build(&mut self, ui: &mut UiFrame) -> Vec<GameEvent> {
        if !self.open {
            return Vec::new();
        }
        let tc = text_color();
        let mut events = Vec::new();

        let menu_h = self.items.len() as f32 * ITEM_H;
        let panel = Rect::new(self.x, self.y, ITEM_W, menu_h);

        ui.begin_popup_layer(panel);
        crate::helper::fallback::panel(ui, panel.x, panel.y, panel.w, panel.h);

        let mut clicked_item = None;
        let mut any_hovered = false;
        for (idx, item) in self.items.iter().enumerate() {
            let iy = self.y + idx as f32 * ITEM_H;
            let rect = Rect::new(self.x, iy, ITEM_W, ITEM_H);
            let resp = ui.interact(WidgetId(MENU_BASE_ID + idx as u32), rect);
            if resp.hovered() {
                any_hovered = true;
                ui.any_interactive_hovered = true;
                let (v, i) =
                    draw::quad_vertices(rect.x, rect.y, rect.w, rect.h, [0.72, 0.79, 0.93, 1.0]);
                ui.draw_calls.push(DrawCall {
                    vertices: v.to_vec(),
                    indices: i.to_vec(),
                    texture: TextureRef::White,
                });
            }
            ui.text(rect.x + 6.0, rect.y + 13.0, &item.label, tc);
            if resp.clicked() {
                clicked_item = Some(item.action.clone());
            }
        }

        if let Some(action) = clicked_item {
            match action {
                ContextMenuAction::InviteToParty { target_aid } => {
                    events.push(GameEvent::RequestPartyInvite { target_aid });
                }
                ContextMenuAction::RequestTrade { target_aid } => {
                    events.push(GameEvent::RequestExchangeItem { target_aid });
                }
                ContextMenuAction::AdoptBaby { target_aid } => {
                    events.push(GameEvent::RequestAdoption { target_aid });
                }
                ContextMenuAction::Whisper { name } => {
                    events.push(GameEvent::RequestWhisper { name });
                }
                ContextMenuAction::ChangeGuildPosition {
                    aid,
                    gid,
                    position_id,
                } => {
                    events.push(GameEvent::RequestChangeMemberPosition {
                        aid,
                        gid,
                        position_id,
                    });
                }
                ContextMenuAction::ExpelFromGuild { aid, gid, name } => {
                    events.push(GameEvent::RequestGuildExpel { aid, gid, name });
                }
                ContextMenuAction::GuildLeave => {
                    events.push(GameEvent::RequestGuildLeave);
                }
                ContextMenuAction::GuildInvite { target_char_id } => {
                    events.push(GameEvent::RequestGuildInvite { target_char_id });
                }
                ContextMenuAction::GuildAlly { target_aid } => {
                    events.push(GameEvent::RequestGuildAlly { target_aid });
                }
                ContextMenuAction::GuildHostile { target_aid } => {
                    events.push(GameEvent::RequestGuildHostile { target_aid });
                }
                ContextMenuAction::CompanionShowInfo { is_mercenary } => {
                    events.push(if is_mercenary {
                        GameEvent::ToggleMercenaryWindow
                    } else {
                        GameEvent::ToggleHomunculusWindow
                    });
                }
                ContextMenuAction::CompanionFeed => {
                    events.push(GameEvent::RequestHomunMenu { command: 1 });
                }
                ContextMenuAction::CompanionStandby { is_mercenary } => {
                    events.push(GameEvent::ToggleCompanionStandby { is_mercenary });
                }
                ContextMenuAction::CompanionPatrol { is_mercenary } => {
                    events.push(GameEvent::ToggleCompanionPatrol { is_mercenary });
                }
                ContextMenuAction::CompanionAiConfig => {
                    events.push(GameEvent::ToggleCompanionAiConfig);
                }
                ContextMenuAction::KickFromChatRoom { name } => {
                    events.push(GameEvent::RequestKickChatMember { name });
                }
                ContextMenuAction::ChangeChatOwner { name } => {
                    events.push(GameEvent::RequestChangeChatOwner { name });
                }
                ContextMenuAction::PetShowInfo => {
                    events.push(GameEvent::RequestPetCommand { csub: 0 });
                }
                ContextMenuAction::PetFeed => {
                    events.push(GameEvent::RequestPetFeed);
                }
                ContextMenuAction::PetCommand { csub } => {
                    events.push(GameEvent::RequestPetCommand { csub });
                }
                ContextMenuAction::GiveMannerPoint {
                    target_aid,
                    positive,
                } => {
                    events.push(GameEvent::RequestMannerPoint {
                        target_aid,
                        positive,
                    });
                }
                ContextMenuAction::AccountName { aid } => {
                    events.push(GameEvent::RequestAccountName { aid });
                }
                ContextMenuAction::ShowInfo { path } => {
                    events.push(GameEvent::OpenReference { path });
                }
                ContextMenuAction::Describe(what) => events.push(match what {
                    Describe::Item { index } => GameEvent::ShowItemInfo { index },
                    Describe::ItemDirect(item) => GameEvent::ShowItemInfoDirect { item },
                    Describe::HotkeySkill { skill, slot } => {
                        GameEvent::DescribeHotkeySkill { skill, slot }
                    }
                }),
            }
            self.close();
        } else if ui.ctx.mouse_pressed && !any_hovered {
            self.close();
        }

        ui.end_popup_layer();
        events
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use ragnarok_ui::context::UiContext;
    use ragnarok_ui::state::StateCache;
    use ragnarok_ui::test_support::test_frame;

    #[test]
    fn info_menu_opens_at_the_pointer() {
        let mut state = StateCache::new();
        let mut ctx = UiContext::new(800.0, 600.0);
        ctx.mouse_x = 30.0;
        ctx.mouse_y = 40.0;
        let ui = test_frame(&mut ctx, &mut state);
        match info_menu(&ui, Some(item_info_path(501)), None) {
            GameEvent::OpenInfoMenu { x, y, info, describe } => {
                assert_eq!((x, y), (30.0, 40.0));
                assert_eq!(info.as_deref(), Some("/db/items/501"));
                assert!(describe.is_none());
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn choosing_info_asks_for_the_database_entry() {
        let mut state = StateCache::new();
        let mut menu = ContextMenu::new();
        menu.open_at(
            100.0,
            100.0,
            vec![ContextMenuItem {
                label: "Info".to_string(),
                action: ContextMenuAction::ShowInfo { path: "/db/mobs/1002".to_string() },
            }],
        );
        let mut ctx = UiContext::new(800.0, 600.0);
        ctx.mouse_x = 110.0;
        ctx.mouse_y = 105.0;
        ctx.mouse_pressed = true;
        ctx.mouse_clicked = true;
        let mut ui = test_frame(&mut ctx, &mut state);
        let events = menu.build(&mut ui);
        assert!(matches!(
            events.as_slice(),
            [GameEvent::OpenReference { path }] if path == "/db/mobs/1002"
        ));
        assert!(!menu.is_open());
    }

    #[test]
    fn a_press_the_world_consumed_still_closes_the_menu() {
        let mut state = StateCache::new();
        let mut menu = ContextMenu::new();
        menu.open_at(
            100.0,
            100.0,
            vec![ContextMenuItem {
                label: "Whisper".to_string(),
                action: ContextMenuAction::Whisper {
                    name: "someone".to_string(),
                },
            }],
        );

        let mut ctx = UiContext::new(800.0, 600.0);
        ctx.mouse_x = 400.0;
        ctx.mouse_y = 400.0;
        ctx.mouse_pressed = true;
        ctx.mouse_clicked = false;
        let mut ui = test_frame(&mut ctx, &mut state);
        menu.build(&mut ui);

        assert!(!menu.is_open());
    }
}
