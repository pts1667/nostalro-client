use std::cell::Cell;
use std::rc::Rc;

use super::confirm_dialog::{ConfirmDialog, ConfirmResult};
use crate::{BuildCtx, InGameWindow, Window};
use ragnarok_game::event::GameEvent;
use ragnarok_ui::frame::{UiFrame, WidgetId, WindowOrder};
use ragnarok_ui::rect::Rect;










const BG_ID: WidgetId = WidgetId(510);
const WINDOW_ID: WidgetId = WidgetId(511);
const RESUME_ID: WidgetId = WidgetId(500);
const OPTION_ID: WidgetId = WidgetId(501);
const CHARSELECT_ID: WidgetId = WidgetId(502);
const QUIT_ID: WidgetId = WidgetId(503);
const RESTART_ID: WidgetId = WidgetId(504);
const RESURRECT_ID: WidgetId = WidgetId(505);
const GRAPHICS_ID: WidgetId = WidgetId(506);
const SHORTCUT_ID: WidgetId = WidgetId(507);
const HOTKEY_ID: WidgetId = WidgetId(508);

const MENU_W: f32 = 140.0;
const FALLBACK_BTN_W: f32 = 120.0;
const FALLBACK_BTN_H: f32 = 24.0;
const BTN_SPACING: f32 = 4.0;
const PADDING_TOP: f32 = 12.0;
const PADDING_BOTTOM: f32 = 12.0;


#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PendingConfirm {
    None,
    CharacterSelect,
    QuitGame,
}

pub struct SystemMenu {
    pub open: bool,
    pub dead: bool,
    pub can_resurrect: bool,
    pending_confirm: PendingConfirm,
    confirm_dialog: ConfirmDialog,
    confirm_dialog_out_param: Rc<Cell<Option<ConfirmResult>>>,
}

impl Default for SystemMenu {
    fn default() -> Self {
        Self::new()
    }
}

impl SystemMenu {
    pub fn new() -> Self {
        Self {
            open: false,
            dead: false,
            can_resurrect: false,
            pending_confirm: PendingConfirm::None,
            confirm_dialog: ConfirmDialog::new(),
            confirm_dialog_out_param: Rc::new(Cell::new(None)),
        }
    }
}

impl Window for SystemMenu {

}

impl InGameWindow for SystemMenu {
    fn owns_keyboard(&self, _ctx: &BuildCtx) -> bool {
        self.open
    }

    fn wants_escape(&self, _ctx: &BuildCtx) -> bool {
        self.open && !self.dead
    }

    fn on_escape(&mut self, _ctx: &mut BuildCtx) -> Vec<GameEvent> {
        if self.pending_confirm != PendingConfirm::None {
            self.pending_confirm = PendingConfirm::None;
        } else {
            self.open = false;
        }
        Vec::new()
    }

    fn build(&mut self, ui: &mut UiFrame, ctx: &mut BuildCtx) -> Vec<GameEvent> {
        let _character = &mut *ctx.character;
        let _data = ctx.data;
        let mut events = Vec::new();

        if !self.open {
            return events;
        }

        if self.pending_confirm != PendingConfirm::None {
            let pending = self.pending_confirm;
            let out_param = Rc::clone(&self.confirm_dialog_out_param);
            self.confirm_dialog_out_param.set(None);
            self.confirm_dialog
                .show("Are you sure?", true, move |result| {
                    out_param.set(Some(result));
                });
            self.confirm_dialog.build(ui);
            if let Some(result) = self.confirm_dialog_out_param.get() {
                match result {
                    ConfirmResult::Ok => {
                        match pending {
                            PendingConfirm::CharacterSelect => {
                                events.push(GameEvent::BackToCharacterSelect)
                            }
                            PendingConfirm::QuitGame => events.push(GameEvent::QuitGame),
                            PendingConfirm::None => {}
                        }
                        self.pending_confirm = PendingConfirm::None;
                        self.open = false;
                    }
                    ConfirmResult::Cancel => {
                        self.pending_confirm = PendingConfirm::None;
                    }
                }
            }
            return events;
        }

        self.build_fallback(ui, &mut events);

        events
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum MenuButton {
    Resume,
    Graphics,
    Sound,
    Shortcut,
    Hotkeys,
    CharSelect,
    Quit,
    Restart,
    Resurrect,
}

impl MenuButton {
    fn id(self) -> WidgetId {
        match self {
            MenuButton::Resume => RESUME_ID,
            MenuButton::Graphics => GRAPHICS_ID,
            MenuButton::Sound => OPTION_ID,
            MenuButton::Shortcut => SHORTCUT_ID,
            MenuButton::Hotkeys => HOTKEY_ID,
            MenuButton::CharSelect => CHARSELECT_ID,
            MenuButton::Quit => QUIT_ID,
            MenuButton::Restart => RESTART_ID,
            MenuButton::Resurrect => RESURRECT_ID,
        }
    }

    fn label(self) -> &'static str {
        match self {
            MenuButton::Resume => "Resume",
            MenuButton::Graphics => "Graphics",
            MenuButton::Sound => "Sound",
            MenuButton::Shortcut => "Shortcut",
            MenuButton::Hotkeys => "Hotkeys",
            MenuButton::CharSelect => "Character Select",
            MenuButton::Quit => "Quit Game",
            MenuButton::Restart => "Restart",
            MenuButton::Resurrect => "Resurrect",
        }
    }
}

impl SystemMenu {
    pub fn open_dead(&mut self) {
        self.open = true;
        self.dead = true;
    }

    pub fn close_dead(&mut self) {
        self.dead = false;
        self.open = false;
    }

    fn buttons(&self) -> Vec<MenuButton> {
        if self.dead {
            let mut buttons = vec![MenuButton::Restart];
            if self.can_resurrect {
                buttons.push(MenuButton::Resurrect);
            }
            buttons.push(MenuButton::CharSelect);
            buttons.push(MenuButton::Quit);
            buttons
        } else {
            vec![
                MenuButton::Resume,
                MenuButton::Graphics,
                MenuButton::Sound,
                MenuButton::Shortcut,
                MenuButton::Hotkeys,
                MenuButton::CharSelect,
                MenuButton::Quit,
            ]
        }
    }

    fn on_button_click(&mut self, button: MenuButton, events: &mut Vec<GameEvent>) {
        match button {
            MenuButton::Resume => self.open = false,
            MenuButton::Graphics => {
                events.push(GameEvent::ToggleGraphicOptions);
                self.open = false;
            }
            MenuButton::Sound => {
                events.push(GameEvent::ToggleSoundOptions);
                self.open = false;
            }
            MenuButton::Shortcut => {
                events.push(GameEvent::ToggleShortcutList);
                self.open = false;
            }
            MenuButton::Hotkeys => {
                events.push(GameEvent::ToggleHotkeyConfig);
                self.open = false;
            }
            MenuButton::CharSelect => self.pending_confirm = PendingConfirm::CharacterSelect,
            MenuButton::Quit => self.pending_confirm = PendingConfirm::QuitGame,
            MenuButton::Restart => events.push(GameEvent::ReturnToSavePoint),
            MenuButton::Resurrect => events.push(GameEvent::RequestStandingResurrection),
        }
    }

    fn claim_pointer(&self, ui: &mut UiFrame, rect: Rect) {
        ui.ensure_in_z_order_with(WINDOW_ID, WindowOrder::Foreground);
        ui.enter_window(WINDOW_ID, rect);
        let screen = Rect::new(0.0, 0.0, ui.ctx.screen_width, ui.ctx.screen_height);
        ui.interact(BG_ID, screen);
    }

    fn build_fallback(&mut self, ui: &mut UiFrame, events: &mut Vec<GameEvent>) {
        let buttons = self.buttons();
        let n = buttons.len() as f32;
        let menu_h = PADDING_TOP + n * FALLBACK_BTN_H + (n - 1.0) * BTN_SPACING + PADDING_BOTTOM;
        let mx = ((ui.ctx.screen_width - MENU_W) / 2.0).floor();
        let top = ((ui.ctx.screen_height - menu_h - crate::helper::fallback::TITLE_H) / 2.0).floor();
        let my = top + crate::helper::fallback::TITLE_H;

        self.claim_pointer(ui, Rect::new(mx, top, MENU_W, menu_h + crate::helper::fallback::TITLE_H));
        crate::helper::fallback::titled_panel(ui, mx, top, MENU_W, menu_h + crate::helper::fallback::TITLE_H);

        let btn_x = mx + (MENU_W - FALLBACK_BTN_W) / 2.0;
        let btn_y = |idx: usize| my + PADDING_TOP + idx as f32 * (FALLBACK_BTN_H + BTN_SPACING);

        for (idx, &button) in buttons.iter().enumerate() {
            let rect = Rect::new(btn_x, btn_y(idx), FALLBACK_BTN_W, FALLBACK_BTN_H);
            if ui
                .button(button.id(), rect, button.label())
                .clicked()
            {
                self.on_button_click(button, events);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::InGameWindow;
    use ragnarok_game::character::Character;
    use ragnarok_game::data_table::DataTable;

    use crate::game::chat_window::CHAT_WINDOW_ID;
    use crate::game::minimap_window::MINIMAP_WINDOW_ID;
    use ragnarok_ui::context::UiContext;
    use ragnarok_ui::state::StateCache;
    use ragnarok_ui::test_support::test_frame;

    #[test]
    fn escape_closes_an_open_menu() {
        let mut menu = SystemMenu::new();
        let mut character = Character::new();
        let data = DataTable::new();
        let mut ctx = crate::BuildCtx::test(&mut character, &data);

        assert!(!menu.wants_escape(&ctx));
        menu.open = true;
        assert!(menu.wants_escape(&ctx));
        menu.on_escape(&mut ctx);
        assert!(!menu.open);
    }

    #[test]
    fn resume_closes_menu() {
        let mut menu = SystemMenu::new();
        menu.open = true;
        let mut state = StateCache::new();
        let mut character = Character::new();
        let data = DataTable::new();

        let mut ctx = UiContext::new(800.0, 600.0);
        let (mx, my) = button_center(600.0, 7, 0);
        ctx.mouse_x = mx;
        ctx.mouse_y = my;
        ctx.mouse_clicked = true;
        let mut ui = test_frame(&mut ctx, &mut state);
        menu.build(&mut ui, &mut crate::BuildCtx::test(&mut character, &data));
        assert!(!menu.open);
    }

    #[test]
    fn graphics_button_emits_toggle_and_closes() {
        let mut menu = SystemMenu::new();
        menu.open = true;
        let mut state = StateCache::new();
        let mut character = Character::new();
        let data = DataTable::new();

        let mut ctx = UiContext::new(800.0, 600.0);
        let (mx, my) = button_center(600.0, 7, 1);
        ctx.mouse_x = mx;
        ctx.mouse_y = my;
        ctx.mouse_clicked = true;
        let mut ui = test_frame(&mut ctx, &mut state);
        let events = menu.build(&mut ui, &mut crate::BuildCtx::test(&mut character, &data));
        assert!(
            events
                .iter()
                .any(|e| matches!(e, GameEvent::ToggleGraphicOptions))
        );
        assert!(!menu.open);
    }

    #[test]
    fn charselect_opens_confirm_then_emits_event() {
        let mut menu = SystemMenu::new();
        menu.open = true;
        let mut state = StateCache::new();
        let mut character = Character::new();
        let data = DataTable::new();

        let mut ctx = UiContext::new(800.0, 600.0);
        let (mx, my) = button_center(600.0, 7, 5);
        ctx.mouse_x = mx;
        ctx.mouse_y = my;
        ctx.mouse_clicked = true;
        let mut ui = test_frame(&mut ctx, &mut state);
        let events = menu.build(&mut ui, &mut crate::BuildCtx::test(&mut character, &data));
        assert!(events.is_empty());
        assert_eq!(menu.pending_confirm, PendingConfirm::CharacterSelect);

        let mut ctx = UiContext::new(800.0, 600.0);
        let dialog_w: f32 = 220.0;
        let dialog_h: f32 = 56.0;
        let dx = ((800.0 - dialog_w) / 2.0).floor();
        let dy = ((600.0 - dialog_h - crate::helper::fallback::TITLE_H) / 2.0).floor() + crate::helper::fallback::TITLE_H;
        let btn_w = 42.0;
        let btn_h = 20.0;
        let btn_x = dx + dialog_w - 5.0 - btn_w * 2.0 - 3.0;
        let btn_y = dy + dialog_h - 4.0 - btn_h;
        ctx.mouse_x = btn_x + btn_w / 2.0;
        ctx.mouse_y = btn_y + btn_h / 2.0;
        ctx.mouse_clicked = true;
        let mut ui = test_frame(&mut ctx, &mut state);
        let events = menu.build(&mut ui, &mut crate::BuildCtx::test(&mut character, &data));
        assert!(
            events
                .iter()
                .any(|e| matches!(e, GameEvent::BackToCharacterSelect))
        );
        assert!(!menu.open);
    }

    #[test]
    fn quit_opens_confirm_then_emits_event() {
        let mut menu = SystemMenu::new();
        menu.open = true;
        let mut state = StateCache::new();
        let mut character = Character::new();
        let data = DataTable::new();

        let mut ctx = UiContext::new(800.0, 600.0);
        let (mx, my) = button_center(600.0, 7, 6);
        ctx.mouse_x = mx;
        ctx.mouse_y = my;
        ctx.mouse_clicked = true;
        let mut ui = test_frame(&mut ctx, &mut state);
        let events = menu.build(&mut ui, &mut crate::BuildCtx::test(&mut character, &data));
        assert!(events.is_empty());
        assert_eq!(menu.pending_confirm, PendingConfirm::QuitGame);

        let mut ctx = UiContext::new(800.0, 600.0);
        let dialog_w: f32 = 220.0;
        let dialog_h: f32 = 56.0;
        let dx = ((800.0 - dialog_w) / 2.0).floor();
        let dy = ((600.0 - dialog_h - crate::helper::fallback::TITLE_H) / 2.0).floor() + crate::helper::fallback::TITLE_H;
        let btn_w = 42.0;
        let btn_h = 20.0;
        let btn_x = dx + dialog_w - 5.0 - btn_w * 2.0 - 3.0;
        let btn_y = dy + dialog_h - 4.0 - btn_h;
        ctx.mouse_x = btn_x + btn_w / 2.0;
        ctx.mouse_y = btn_y + btn_h / 2.0;
        ctx.mouse_clicked = true;
        let mut ui = test_frame(&mut ctx, &mut state);
        let events = menu.build(&mut ui, &mut crate::BuildCtx::test(&mut character, &data));
        assert!(events.iter().any(|e| matches!(e, GameEvent::QuitGame)));
        assert!(!menu.open);
    }

    #[test]
    fn confirm_cancel_returns_to_menu() {
        let mut menu = SystemMenu::new();
        menu.open = true;
        menu.pending_confirm = PendingConfirm::CharacterSelect;
        let mut state = StateCache::new();
        let mut character = Character::new();
        let data = DataTable::new();

        let mut ctx = UiContext::new(800.0, 600.0);
        let dialog_w: f32 = 220.0;
        let dialog_h: f32 = 56.0;
        let dx = ((800.0 - dialog_w) / 2.0).floor();
        let dy = ((600.0 - dialog_h - crate::helper::fallback::TITLE_H) / 2.0).floor() + crate::helper::fallback::TITLE_H;
        let btn_w = 42.0;
        let btn_h = 20.0;
        let cancel_btn_x = dx + dialog_w - 5.0 - btn_w;
        let cancel_btn_y = dy + dialog_h - 4.0 - btn_h;
        ctx.mouse_x = cancel_btn_x + btn_w / 2.0;
        ctx.mouse_y = cancel_btn_y + btn_h / 2.0;
        ctx.mouse_clicked = true;
        let mut ui = test_frame(&mut ctx, &mut state);
        let events = menu.build(&mut ui, &mut crate::BuildCtx::test(&mut character, &data));
        assert!(events.is_empty());
        assert_eq!(menu.pending_confirm, PendingConfirm::None);
        assert!(menu.open);
    }

    fn button_center(screen_h: f32, button_count: usize, idx: usize) -> (f32, f32) {
        let n = button_count as f32;
        let menu_h = PADDING_TOP + n * FALLBACK_BTN_H + (n - 1.0) * BTN_SPACING + PADDING_BOTTOM;
        let btn_x = ((800.0 - MENU_W) / 2.0).floor() + (MENU_W - FALLBACK_BTN_W) / 2.0;
        let my = ((screen_h - menu_h - crate::helper::fallback::TITLE_H) / 2.0).floor() + crate::helper::fallback::TITLE_H;
        let btn_y = my + PADDING_TOP + idx as f32 * (FALLBACK_BTN_H + BTN_SPACING);
        (btn_x + FALLBACK_BTN_W / 2.0, btn_y + FALLBACK_BTN_H / 2.0)
    }

    #[test]
    fn button_wins_over_a_window_drawn_behind_the_menu() {
        let mut menu = SystemMenu::new();
        menu.open = true;
        let mut state = StateCache::new();
        let mut character = Character::new();
        let data = DataTable::new();
        let (mx, my) = button_center(600.0, 7, 0);

        let mut frame = |clicked: bool| {
            let mut ctx = UiContext::new(800.0, 600.0);
            ctx.mouse_x = mx;
            ctx.mouse_y = my;
            ctx.mouse_clicked = clicked;
            let mut ui = test_frame(&mut ctx, &mut state);
            let z = ui.get_z_order();
            ui.compute_hovered_window(&z);
            ui.window_fixed(CHAT_WINDOW_ID, 800.0, 600.0, 0.0, 0.0);
            ui.window_fixed(MINIMAP_WINDOW_ID, 100.0, 100.0, 700.0, 0.0);
            menu.build(&mut ui, &mut crate::BuildCtx::test(&mut character, &data));
        };

        frame(false);
        frame(true);
        assert!(!menu.open);
    }

    #[test]
    fn dead_restart_emits_return_to_savepoint() {
        let mut menu = SystemMenu::new();
        menu.open_dead();
        let mut state = StateCache::new();
        let mut character = Character::new();
        let data = DataTable::new();

        let (mx, my) = button_center(600.0, 3, 0);
        let mut ctx = UiContext::new(800.0, 600.0);
        ctx.mouse_x = mx;
        ctx.mouse_y = my;
        ctx.mouse_clicked = true;
        let mut ui = test_frame(&mut ctx, &mut state);
        let events = menu.build(&mut ui, &mut crate::BuildCtx::test(&mut character, &data));
        assert!(
            events
                .iter()
                .any(|e| matches!(e, GameEvent::ReturnToSavePoint))
        );
    }

    #[test]
    fn resurrect_button_present_only_with_token() {
        let mut menu = SystemMenu::new();
        menu.open_dead();
        menu.can_resurrect = true;
        let mut state = StateCache::new();
        let mut character = Character::new();
        let data = DataTable::new();

        let (mx, my) = button_center(600.0, 4, 1);
        let mut ctx = UiContext::new(800.0, 600.0);
        ctx.mouse_x = mx;
        ctx.mouse_y = my;
        ctx.mouse_clicked = true;
        let mut ui = test_frame(&mut ctx, &mut state);
        let events = menu.build(&mut ui, &mut crate::BuildCtx::test(&mut character, &data));
        assert!(
            events
                .iter()
                .any(|e| matches!(e, GameEvent::RequestStandingResurrection))
        );
    }

    #[test]
    fn escape_cannot_close_death_menu() {
        let mut menu = SystemMenu::new();
        menu.open_dead();
        let mut character = Character::new();
        let data = DataTable::new();
        let ctx = crate::BuildCtx::test(&mut character, &data);

        assert!(!menu.wants_escape(&ctx));
        assert!(menu.open);
        assert!(menu.dead);
    }
}
