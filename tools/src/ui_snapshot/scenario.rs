//! Scripted input for snapshots. Each step runs as one or more UI frames, so a
//! hover, click or keypress lands the same way it does in the client.

use ragnarok_ui::context::UiContext;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Key {
    Enter,
    Tab,
    Escape,
    Backspace,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Step {
    /// Move the pointer. One frame.
    Hover { x: f32, y: f32 },
    /// Move the pointer, press, then release. Two frames: the press frame
    /// carries the click pulse, and the release frame lets the press settle.
    Click { x: f32, y: f32 },
    /// Type text. One frame, all characters in `typed_chars`.
    Type(String),
    /// Press one named key. One frame.
    Key(Key),
}

impl Step {
    /// Sets the input fields this step needs for its first frame.
    pub fn apply(&self, ctx: &mut UiContext) {
        match self {
            Step::Hover { x, y } => {
                ctx.mouse_x = *x;
                ctx.mouse_y = *y;
            }
            Step::Click { x, y } => {
                ctx.mouse_x = *x;
                ctx.mouse_y = *y;
                ctx.mouse_clicked = true;
                ctx.mouse_pressed = true;
                ctx.mouse_down = true;
            }
            Step::Type(text) => {
                ctx.typed_chars.extend(text.chars());
            }
            Step::Key(key) => match key {
                Key::Enter => ctx.key_enter = true,
                Key::Tab => ctx.key_tab = true,
                Key::Escape => ctx.key_escape = true,
                Key::Backspace => ctx.key_backspace = true,
            },
        }
    }

    pub fn is_click(&self) -> bool {
        matches!(self, Step::Click { .. })
    }
}

pub fn parse_key(s: &str) -> Result<Key, String> {
    match s.to_ascii_lowercase().as_str() {
        "enter" => Ok(Key::Enter),
        "tab" => Ok(Key::Tab),
        "escape" | "esc" => Ok(Key::Escape),
        "backspace" => Ok(Key::Backspace),
        other => Err(format!(
            "unknown key '{other}' (expected enter, tab, escape or backspace)"
        )),
    }
}

/// Parses `X,Y` into logical pixels.
pub fn parse_point(s: &str) -> Result<(f32, f32), String> {
    let (x, y) = s
        .split_once(',')
        .ok_or_else(|| format!("expected X,Y but got '{s}'"))?;
    let x: f32 = x.trim().parse().map_err(|_| format!("bad x in '{s}'"))?;
    let y: f32 = y.trim().parse().map_err(|_| format!("bad y in '{s}'"))?;
    Ok((x, y))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_points_and_keys() {
        assert_eq!(parse_point("40,60"), Ok((40.0, 60.0)));
        assert_eq!(parse_point(" 4.5 , 6 "), Ok((4.5, 6.0)));
        assert!(parse_point("40").is_err());
        assert!(parse_point("a,b").is_err());
        assert_eq!(parse_key("ESC"), Ok(Key::Escape));
        assert!(parse_key("f13").is_err());
    }

    #[test]
    fn click_sets_press_fields() {
        let mut ctx = UiContext::new(320.0, 240.0);
        Step::Click { x: 10.0, y: 20.0 }.apply(&mut ctx);
        assert_eq!((ctx.mouse_x, ctx.mouse_y), (10.0, 20.0));
        assert!(ctx.mouse_clicked && ctx.mouse_pressed && ctx.mouse_down);
    }

    #[test]
    fn type_appends_characters() {
        let mut ctx = UiContext::new(320.0, 240.0);
        Step::Type("ab".into()).apply(&mut ctx);
        assert_eq!(ctx.typed_chars, vec!['a', 'b']);
    }
}
