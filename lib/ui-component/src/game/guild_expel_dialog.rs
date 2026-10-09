use super::input_dialog::{InputDialog, InputDialogConfig, InputDialogLayout, InputDialogResult};
use crate::{BuildCtx, InGameWindow, Window};
use ragnarok_game::event::GameEvent;
use ragnarok_ui::frame::{UiFrame, WidgetId};

const REASON_MAX_LEN: usize = 39;
const BASE_ID: WidgetId = WidgetId(425);

pub struct GuildExpelDialog {
    pub aid: u32,
    pub gid: u32,
    pub name: String,
    inner: InputDialog,
}

impl GuildExpelDialog {
    pub fn new(aid: u32, gid: u32, name: String) -> Self {
        let config = InputDialogConfig {
            layout: InputDialogLayout::Text {
                label: Some(format!("Reason for expelling {name}:")),
                show_cancel: true,
            },
            escape_cancels: true,
            default_value: String::new(),
            max_len: REASON_MAX_LEN,
            numeric_only: false,
            max_value: None,
        };
        Self {
            aid,
            gid,
            name,
            inner: InputDialog::new(config, BASE_ID),
        }
    }
}

impl InGameWindow for GuildExpelDialog {
    fn owns_keyboard(&self, _ctx: &BuildCtx) -> bool {
        true
    }

    fn wants_escape(&self, _ctx: &BuildCtx) -> bool {
        true
    }

    fn on_escape(&mut self, _ctx: &mut BuildCtx) -> Vec<GameEvent> {
        Vec::new()
    }

    fn build(&mut self, ui: &mut UiFrame, ctx: &mut BuildCtx) -> Vec<GameEvent> {
        let _character = &mut *ctx.character;
        let _data = ctx.data;
        match self.inner.build(ui) {
            InputDialogResult::Submitted => vec![GameEvent::ConfirmedGuildExpel {
                aid: self.aid,
                gid: self.gid,
                name: self.name.clone(),
                reason: self.inner.value_str().to_string(),
            }],
            InputDialogResult::Cancel => vec![GameEvent::DialogClosed],
            InputDialogResult::None => vec![],
        }
    }
}

impl Window for GuildExpelDialog {}

#[cfg(test)]
mod tests {
    use super::*;
    use ragnarok_game::character::Character;
    use ragnarok_game::data_table::DataTable;

    use ragnarok_ui::context::UiContext;
    use ragnarok_ui::state::StateCache;
    use ragnarok_ui::test_support::test_frame;

    fn build(
        dialog: &mut GuildExpelDialog,
        ctx: &mut UiContext,
        state: &mut StateCache,
    ) -> Vec<GameEvent> {
        let mut ui = test_frame(ctx, state);
        let mut character = Character::new();
        dialog.build(
            &mut ui,
            &mut crate::BuildCtx::test(&mut character, &DataTable::default()),
        )
    }

    #[test]
    fn typed_reason_is_sent_on_submit() {
        let mut dialog = GuildExpelDialog::new(11, 22, "Bob".to_string());
        let mut state = StateCache::new();

        let mut ctx = UiContext::new(800.0, 600.0);
        ctx.typed_chars = "afk".chars().collect();
        let _ = build(&mut dialog, &mut ctx, &mut state);

        let mut ctx = UiContext::new(800.0, 600.0);
        ctx.key_enter = true;
        let events = build(&mut dialog, &mut ctx, &mut state);

        assert!(matches!(
            events.as_slice(),
            [GameEvent::ConfirmedGuildExpel { aid: 11, gid: 22, name, reason }]
                if name == "Bob" && reason == "afk"
        ));
    }

    #[test]
    fn escape_closes_without_expel() {
        let mut dialog = GuildExpelDialog::new(1, 2, "X".to_string());
        let mut state = StateCache::new();
        let mut ctx = UiContext::new(800.0, 600.0);
        ctx.key_escape = true;
        let events = build(&mut dialog, &mut ctx, &mut state);
        assert!(matches!(events.as_slice(), [GameEvent::DialogClosed]));
    }
}
