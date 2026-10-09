use ragnarok_ui::draw::DrawCall;

/// Frame behind dialog content. Drawn as the light panel from `helper::fallback`.
#[derive(Default)]
pub struct DialogContainer;

impl DialogContainer {
    pub fn new() -> Self {
        Self
    }

    /// Dark text on the light panel.
    pub fn text_color(&self) -> [f32; 4] {
        [0.0, 0.0, 0.0, 1.0]
    }

    pub fn draw(
        &self,
        draw_calls: &mut Vec<DrawCall>,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        _color: [f32; 4],
    ) {
        draw_fallback(draw_calls, x, y, w, h);
    }
}

/// The light panel from `helper::fallback`.
fn draw_fallback(draw_calls: &mut Vec<DrawCall>, x: f32, y: f32, w: f32, h: f32) {
    crate::helper::fallback::panel_into(draw_calls, x, y, w, h);
}

#[cfg(test)]
mod tests {
    use super::*;
    use ragnarok_ui::draw::TextureRef;

    #[test]
    fn fallback_draws_background_and_border() {
        let mut calls = Vec::new();
        draw_fallback(&mut calls, 0.0, 0.0, 100.0, 80.0);
        assert_eq!(calls.len(), 2);
        for call in &calls {
            assert!(matches!(call.texture, TextureRef::White));
        }
    }

    #[test]
    fn draw_uses_fallback_panel() {
        let container = DialogContainer::new();
        let mut calls = Vec::new();
        container.draw(&mut calls, 0.0, 0.0, 100.0, 80.0, [1.0; 4]);
        assert_eq!(calls.len(), 2);
    }

    #[test]
    fn text_color_is_dark() {
        assert_eq!(DialogContainer::new().text_color(), [0.0, 0.0, 0.0, 1.0]);
    }
}
