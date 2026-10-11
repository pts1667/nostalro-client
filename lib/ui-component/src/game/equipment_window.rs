use super::inventory_window::INV_WINDOW_ID;
use crate::helper::window_chrome::{draw_sys_button, draw_titlebar, text_color};
use crate::{BuildCtx, InGameWindow, Window};
use ragnarok_game::display_name::format_equipment_display_name_colored;
use crate::game::context_menu::{info_menu, item_info_path};
use ragnarok_game::event::{Describe, GameEvent};
use ragnarok_game::inventory::{EquipmentLocation, InventoryData};
use ragnarok_game::sprite_path::OPTION_REMOVABLE_MASK;
use ragnarok_ui::draw::{self, DrawCall, TextureRef};
use ragnarok_ui::frame::{UiFrame, WidgetId};
use ragnarok_ui::rect::Rect;
use ragnarok_ui::theme::FallbackPalette;

pub const EQ_WINDOW_ID: WidgetId = WidgetId(900);
const EQ_CLOSE_BTN_ID: WidgetId = WidgetId(901);
const EQ_MINI_BTN_ID: WidgetId = WidgetId(902);
const EQ_REMOVE_OPTION_BTN_ID: WidgetId = WidgetId(903);
const EQ_CART_SLOT_ID: WidgetId = WidgetId(904);
const EQ_SLOT_BASE_ID: u32 = 910;

const TITLE_H: f32 = 17.0;
const WIN_W: f32 = 280.0;
const CONTENT_H: f32 = 130.0;
const MINI_BTN_SIZE: f32 = 11.0;

const SIDE_COL_W: f32 = 115.0;
const CENTER_COL_W: f32 = 50.0;
const ICON_SIZE: f32 = 24.0;
const EMPTY_LABEL: [f32; 4] = [0.55, 0.60, 0.70, 1.0];
const SLOT_ROWS: usize = 5;
const TEXT_MAX_W: f32 = SIDE_COL_W - ICON_SIZE - 4.0 - 3.0;
const REMOVE_OPTION_BTN_SIZE: f32 = 36.0;

struct EquipSlot {
    label: fn() -> String,
    location: EquipmentLocation,
    col: u8, // 0=left, 1=right, 2=center
    row: u8,
}

const EQUIP_SLOTS: &[EquipSlot] = &[
    EquipSlot {
        label: || rust_i18n::t!("equipment.slot_head_top").to_string(),
        location: EquipmentLocation::HeadTop,
        col: 0,
        row: 0,
    },
    EquipSlot {
        label: || rust_i18n::t!("equipment.slot_head_low").to_string(),
        location: EquipmentLocation::HeadLow,
        col: 0,
        row: 1,
    },
    EquipSlot {
        label: || rust_i18n::t!("equipment.slot_weapon").to_string(),
        location: EquipmentLocation::HandRight,
        col: 0,
        row: 2,
    },
    EquipSlot {
        label: || rust_i18n::t!("equipment.slot_garment").to_string(),
        location: EquipmentLocation::Garment,
        col: 0,
        row: 3,
    },
    EquipSlot {
        label: || rust_i18n::t!("equipment.slot_accessory_left").to_string(),
        location: EquipmentLocation::AccessoryLeft,
        col: 0,
        row: 4,
    },
    EquipSlot {
        label: || rust_i18n::t!("equipment.slot_head_mid").to_string(),
        location: EquipmentLocation::HeadMid,
        col: 1,
        row: 0,
    },
    EquipSlot {
        label: || rust_i18n::t!("equipment.slot_armor").to_string(),
        location: EquipmentLocation::Armor,
        col: 1,
        row: 1,
    },
    EquipSlot {
        label: || rust_i18n::t!("equipment.slot_shield").to_string(),
        location: EquipmentLocation::HandLeft,
        col: 1,
        row: 2,
    },
    EquipSlot {
        label: || rust_i18n::t!("equipment.slot_shoes").to_string(),
        location: EquipmentLocation::Shoes,
        col: 1,
        row: 3,
    },
    EquipSlot {
        label: || rust_i18n::t!("equipment.slot_accessory_right").to_string(),
        location: EquipmentLocation::AccessoryRight,
        col: 1,
        row: 4,
    },
    EquipSlot {
        label: || rust_i18n::t!("equipment.slot_ammo").to_string(),
        location: EquipmentLocation::Ammo,
        col: 2,
        row: 0,
    },
];

pub struct EquipmentWindow {
    pub open: bool,
    minimized: bool,
    bg_size: (f32, f32),
    character_center: Option<[f32; 2]>,
    paperdoll_insert_index: Option<usize>,
    cart_slot_center: Option<[f32; 2]>,
}

impl Default for EquipmentWindow {
    fn default() -> Self {
        Self::new()
    }
}

impl EquipmentWindow {
    pub fn new() -> Self {
        Self {
            open: false,
            minimized: false,
            bg_size: (0.0, 0.0),
            character_center: None,
            paperdoll_insert_index: None,
            cart_slot_center: None,
        }
    }

    pub fn character_center(&self) -> Option<[f32; 2]> {
        self.character_center
    }

    pub fn paperdoll_insert_index(&self) -> Option<usize> {
        self.paperdoll_insert_index
    }

    pub fn cart_slot_center(&self) -> Option<[f32; 2]> {
        self.cart_slot_center
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn is_visible(&self) -> bool {
        self.open && !self.minimized
    }

    pub fn is_minimized(&self) -> bool {
        self.minimized
    }

    pub fn set_minimized(&mut self, value: bool) {
        self.minimized = value;
    }

    pub fn toggle(&mut self) {
        self.open = !self.open;
    }
}

impl Window for EquipmentWindow {
    fn window_size(&self) -> (f32, f32) {
        let content_h = if self.bg_size.1 > 0.0 {
            self.bg_size.1
        } else {
            CONTENT_H
        };
        (WIN_W, TITLE_H + content_h)
    }
}

impl InGameWindow for EquipmentWindow {
    fn wants_escape(&self, _ctx: &BuildCtx) -> bool {
        self.open
    }

    fn on_escape(&mut self, _ctx: &mut BuildCtx) -> Vec<GameEvent> {
        self.open = false;
        Vec::new()
    }

    fn build(&mut self, ui: &mut UiFrame, ctx: &mut BuildCtx) -> Vec<GameEvent> {
        let character = &mut *ctx.character;
        let data = ctx.data;
        let inventory = &character.inventory;
        let producers = &character.char_names;
        self.character_center = None;
        self.paperdoll_insert_index = None;
        self.cart_slot_center = None;

        if !self.open {
            return Vec::new();
        }

        let mut events = Vec::new();

        let text_color = text_color();

        let win_w = WIN_W;
        let content_h = if self.bg_size.1 > 0.0 {
            self.bg_size.1
        } else {
            CONTENT_H
        };
        let win_h = if self.minimized {
            TITLE_H
        } else {
            (TITLE_H) + content_h
        };

        let default_x = 0.0;
        let default_y = 210.0;
        let win = ui.window_at(EQ_WINDOW_ID, win_w, win_h, TITLE_H, default_x, default_y);

        let win_rect = Rect::new(win.x, win.y, win_w, win_h);
        ui.interact(EQ_WINDOW_ID, win_rect);

        draw_titlebar(ui, win.x, win.y, win_w, TITLE_H);
        ui.text(
            win.x + (17.0),
            win.y + (TITLE_H) - (3.0),
            &rust_i18n::t!("equipment.equipment"),
            text_color,
        );

        let btn_size = MINI_BTN_SIZE;
        let mini_rect = Rect::new(
            win.x + win_w - btn_size * 2.0 - (6.0),
            win.y + ((TITLE_H) - btn_size) / 2.0,
            btn_size,
            btn_size,
        );
        let mini_resp = ui.interact(EQ_MINI_BTN_ID, mini_rect);
        if mini_resp.hovered() {
            ui.any_interactive_hovered = true;
        }
        draw_sys_button(
            ui,
            mini_rect,
            (btn_size, btn_size),
            mini_resp.hovered(),
            Some('_'),
        );
        if mini_resp.clicked() {
            self.minimized = !self.minimized;
        }

        let close_rect = Rect::new(
            win.x + win_w - btn_size - (3.0),
            win.y + ((TITLE_H) - btn_size) / 2.0,
            btn_size,
            btn_size,
        );
        let close_resp = ui.interact(EQ_CLOSE_BTN_ID, close_rect);
        if close_resp.hovered() {
            ui.any_interactive_hovered = true;
        }
        draw_sys_button(
            ui,
            close_rect,
            (btn_size, btn_size),
            close_resp.hovered(),
            Some('x'),
        );
        if close_resp.clicked() {
            self.open = false;
            return events;
        }

        if self.minimized {
            return events;
        }

        let content_y = win.y + (TITLE_H);
        crate::helper::fallback::window_body(ui, win.x, content_y, win_w, content_h);

        let slot_h = content_h / SLOT_ROWS as f32;
        let character_x = win.x + SIDE_COL_W + CENTER_COL_W / 2.0;
        let character_y = content_y + content_h - slot_h;
        self.character_center = Some([character_x, character_y]);
        self.paperdoll_insert_index = Some(ui.draw_calls.len());

        if character.cart_design.is_none() && character.effect_state & OPTION_REMOVABLE_MASK != 0 {
            let btn_x = win.x + SIDE_COL_W + (CENTER_COL_W - REMOVE_OPTION_BTN_SIZE) / 2.0;
            let btn_y = win.y + content_h - slot_h;
            let btn_rect = Rect::new(btn_x, btn_y, REMOVE_OPTION_BTN_SIZE, REMOVE_OPTION_BTN_SIZE);
            let resp = ui.interact(EQ_REMOVE_OPTION_BTN_ID, btn_rect);
            if resp.hovered() {
                ui.any_interactive_hovered = true;
            }
            let color = if resp.hovered() {
                [0.6, 0.2, 0.2, 1.0]
            } else {
                [0.4, 0.15, 0.15, 1.0]
            };
            let (v, idx) =
                draw::quad_vertices(btn_rect.x, btn_rect.y, btn_rect.w, btn_rect.h, color);
            ui.draw_calls.push(DrawCall {
                vertices: v.to_vec(),
                indices: idx.to_vec(),
                texture: TextureRef::White,
            });
            ui.text(
                btn_rect.x + 8.0,
                btn_rect.y + 12.0,
                &rust_i18n::t!("equipment.off"),
                [1.0, 1.0, 1.0, 1.0],
            );
            if resp.clicked() {
                events.push(GameEvent::RequestRemoveOption);
            }
        }

        if character.cart_design.is_some() {
            let slot_w = CENTER_COL_W;
            let slot_x = win.x + SIDE_COL_W;
            let slot_y = content_y + content_h - slot_h;
            let cart_rect = Rect::new(slot_x, slot_y, slot_w, slot_h);
            self.cart_slot_center = Some([slot_x + slot_w / 2.0, content_y + content_h - 4.0]);
            let resp = ui.interact(EQ_CART_SLOT_ID, cart_rect);
            if resp.hovered() {
                ui.any_interactive_hovered = true;
            }
            if resp.clicked() {
                character.cart.open();
            }
            if resp.right_clicked() {
                events.push(GameEvent::RequestRemoveOption);
            }
            if resp.hovered() {
                ui.tooltip(
                    cart_rect.x,
                    cart_rect.y - ui.atlas.line_height - 4.0,
                    &rust_i18n::t!("equipment.cart_click_to_open_right_click_to_remove"),
                );
            }
        }

        let highlight_location: Option<u16> = ui
            .drag_info()
            .filter(|(src, _)| *src == INV_WINDOW_ID)
            .and_then(|(_, idx)| inventory.get_item(idx as u16))
            .filter(|item| item.is_equipment() && !item.is_equipped())
            .map(|item| item.equip_location());

        let side_col_w = SIDE_COL_W;
        let icon = ICON_SIZE;

        for (i, slot) in EQUIP_SLOTS.iter().enumerate() {
            let (slot_x, slot_w) = match slot.col {
                0 => (win.x, side_col_w),                      // Left column
                1 => (win.x + win_w - side_col_w, side_col_w), // Right column
                _ => (win.x + side_col_w, (CENTER_COL_W)),     // Center column
            };
            let slot_y = content_y + slot.row as f32 * slot_h;

            let widget_id = WidgetId(EQ_SLOT_BASE_ID + i as u32);

            let icon_pad = (slot_h - icon) / 2.0;
            let (icon_x, text_x, right_align, show_text) = match slot.col {
                0 => {
                    let ix = slot_x + (4.0);
                    let tx = ix + icon + (3.0);
                    (ix, tx, false, true)
                }
                1 => {
                    let ix = slot_x + slot_w - icon - (4.0);
                    (ix, ix - 3.0, true, true)
                }
                _ => {
                    let ix = slot_x + (slot_w - icon) / 2.0;
                    (ix, ix, false, false)
                }
            };
            let icon_y = slot_y + icon_pad;
            let icon_rect = Rect::new(icon_x, icon_y, icon, icon);
            let response = ui.interact(widget_id, icon_rect);

            // A framed cell marks every slot; an empty side slot also names itself.
            if slot.col != 2 && slot.row > 0 {
                let (v, i) = draw::quad_vertices(
                    slot_x + 2.0,
                    slot_y,
                    slot_w - 4.0,
                    1.0,
                    FallbackPalette::WIN_BORDER,
                );
                ui.draw_calls.push(DrawCall {
                    vertices: v.to_vec(),
                    indices: i.to_vec(),
                    texture: TextureRef::White,
                });
            }
            crate::helper::fallback::slot_cell(ui, icon_rect.x, icon_rect.y, icon, icon);
            if show_text && inventory.equipped_in_slot(slot.location).is_none() {
                let label = (slot.label)();
                let label_w = ui.atlas.measure_text(&label);
                let lx = if right_align { text_x - label_w } else { text_x };
                ui.text(lx, slot_y + slot_h / 2.0 + 4.0, &label, EMPTY_LABEL);
            }

            if let Some(item) = inventory.equipped_in_slot(slot.location) {
                if let Some(icon_path) = item.icon_path() {
                    let (v, idx) = draw::quad_vertices(
                        icon_rect.x,
                        icon_rect.y,
                        icon_rect.w,
                        icon_rect.h,
                        [1.0, 1.0, 1.0, 1.0],
                    );
                    ui.draw_calls.push(DrawCall {
                        vertices: v.to_vec(),
                        indices: idx.to_vec(),
                        texture: TextureRef::Named(icon_path),
                    });
                }

                let display_name =
                    format_equipment_display_name_colored(item, data, producers, text_color);
                if show_text {
                    let mut lines = draw::colored_word_wrap(
                        &display_name,
                        TEXT_MAX_W,
                        |t| ui.atlas.measure_text(t),
                        true,
                    );
                    lines.reverse();
                    let line_h = ui.atlas.line_height - 2.0;
                    let mut block_bottom = slot_y + slot_h;

                    if lines.len() == 3 {
                        block_bottom += 8.0;
                    }

                    let line_space = 1.0 * lines.len() as f32 - 1.0;
                    for (j, line) in lines.iter().enumerate() {
                        let ly = block_bottom - (j as f32 + 1.0) * (line_h - line_space)
                            + (line_h - line_space) / 2.0;
                        if right_align {
                            let text_w = ui.atlas.measure_text(&draw::strip_color_codes(line));
                            ui.colored_text(text_x - text_w, ly, line, text_color);
                        } else {
                            ui.colored_text(text_x, ly, line, text_color);
                        }
                    }
                }

                let mut clicked = false;

                if response.clicked() {
                    ui.drag_source(
                        EQ_WINDOW_ID,
                        item.index as usize,
                        item.icon_path(),
                        (ICON_SIZE, ICON_SIZE),
                    );
                    clicked = true;
                }

                if response.right_clicked() {
                    events.push(info_menu(
                        ui,
                        Some(item_info_path(item.item_id)),
                        Some(Describe::Item { index: item.index }),
                    ));
                    clicked = true;
                }
                if response.double_clicked() {
                    events.push(GameEvent::RequestUnequipItem { index: item.index });
                    clicked = true;
                }
                if !clicked && response.hovered() {
                    let tooltip = if item.count > 1 {
                        format!("{}: {} ea.", display_name, item.count)
                    } else {
                        draw::strip_color_codes(&display_name)
                    };
                    ui.tooltip(
                        icon_rect.x,
                        icon_rect.y - ui.atlas.line_height - 16.0,
                        &tooltip,
                    );
                }
            }

            if let Some(loc) = highlight_location
                && loc & InventoryData::slot_mask(slot.location) != 0
            {
                let (v, idx) = draw::quad_vertices(
                    icon_rect.x,
                    icon_rect.y,
                    icon_rect.w,
                    icon_rect.h,
                    [0.20, 0.42, 0.88, 0.35],
                );
                ui.draw_calls.push(DrawCall {
                    vertices: v.to_vec(),
                    indices: idx.to_vec(),
                    texture: TextureRef::White,
                });
            }
        }

        let content_rect = Rect::new(win.x, content_y, win_w, content_h);
        if let Some((source_id, item_index)) = ui.drop_zone(content_rect)
            && source_id == INV_WINDOW_ID
            && let Some(item) = inventory.get_item(item_index as u16)
            && item.is_equipment()
            && !item.is_equipped()
        {
            events.push(GameEvent::RequestEquipItem {
                index: item.index,
                location: item.equip_location(),
            });
        }

        events
    }
}
