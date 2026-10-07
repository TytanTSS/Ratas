//! The keys of the quick-access cells: the ability cells of the hotbar (as
//! many as the hero chose, up to HOTBAR_MAX) and the two potions. Every cell
//! has up to two bindings — a key or a mouse button. They live in the config
//! (Config::hotkeys) and are changed in the controls window.

use macroquad::prelude::*;

use ratas_core::config::Config;
use ratas_core::game::HOTBAR_MAX;

use crate::gfx::tr;
use crate::ui::UiInput;

pub const POTION_HEALTH: usize = HOTBAR_MAX;
pub const POTION_MANA: usize = HOTBAR_MAX + 1;
/// The ability cells and the two potions.
pub const CELLS: usize = HOTBAR_MAX + 2;
/// Bindings per cell.
pub const BINDS: usize = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bind {
    Key(KeyCode),
    Mouse(MouseButton),
}

/// The keys that can be bound, with their names (in the config and on
/// screen). The rest are kept by the game: see RESERVED.
const KEYS: &[(KeyCode, &str)] = &[
    (KeyCode::Key1, "1"),
    (KeyCode::Key2, "2"),
    (KeyCode::Key3, "3"),
    (KeyCode::Key4, "4"),
    (KeyCode::Key5, "5"),
    (KeyCode::Key6, "6"),
    (KeyCode::Key7, "7"),
    (KeyCode::Key8, "8"),
    (KeyCode::Key9, "9"),
    (KeyCode::Key0, "0"),
    (KeyCode::Q, "Q"),
    (KeyCode::R, "R"),
    (KeyCode::Z, "Z"),
    (KeyCode::X, "X"),
    (KeyCode::V, "V"),
    (KeyCode::B, "B"),
    (KeyCode::N, "N"),
    (KeyCode::O, "O"),
    (KeyCode::P, "P"),
    (KeyCode::Y, "Y"),
    (KeyCode::F2, "F2"),
    (KeyCode::F3, "F3"),
    (KeyCode::F4, "F4"),
    (KeyCode::F6, "F6"),
    (KeyCode::F7, "F7"),
    (KeyCode::F8, "F8"),
    (KeyCode::F10, "F10"),
    (KeyCode::Kp1, "Num 1"),
    (KeyCode::Kp2, "Num 2"),
    (KeyCode::Kp3, "Num 3"),
    (KeyCode::Kp4, "Num 4"),
    (KeyCode::Kp5, "Num 5"),
    (KeyCode::Kp6, "Num 6"),
    (KeyCode::Kp7, "Num 7"),
    (KeyCode::Kp8, "Num 8"),
    (KeyCode::Kp9, "Num 9"),
    (KeyCode::Kp0, "Num 0"),
    (KeyCode::KpMultiply, "Num *"),
    (KeyCode::KpDivide, "Num /"),
    (KeyCode::KpDecimal, "Num ."),
    (KeyCode::LeftBracket, "["),
    (KeyCode::RightBracket, "]"),
    (KeyCode::Semicolon, ";"),
    (KeyCode::Apostrophe, "'"),
    (KeyCode::Comma, ","),
    (KeyCode::Period, "."),
    (KeyCode::Backslash, "\\"),
    (KeyCode::GraveAccent, "`"),
    (KeyCode::LeftShift, "Shift"),
    (KeyCode::RightShift, "Right Shift"),
    (KeyCode::LeftAlt, "Alt"),
    (KeyCode::RightAlt, "Right Alt"),
    (KeyCode::CapsLock, "Caps Lock"),
    (KeyCode::Insert, "Insert"),
    (KeyCode::Home, "Home"),
    (KeyCode::End, "End"),
    (KeyCode::PageUp, "Page Up"),
    (KeyCode::PageDown, "Page Down"),
];

/// Modifier keys act only in the game itself, not in windows (Shift+Tab).
const MODIFIERS: &[KeyCode] = &[
    KeyCode::LeftShift,
    KeyCode::RightShift,
    KeyCode::LeftAlt,
    KeyCode::RightAlt,
];

const MOUSE: &[(MouseButton, &str, &str)] = &[
    (MouseButton::Right, "MouseRight", "ПКМ"),
    (MouseButton::Middle, "MouseMiddle", "СКМ"),
];

/// Keys the game keeps for itself, and what for.
const RESERVED: &[(KeyCode, &str, &str)] = &[
    (KeyCode::W, "W", "ходьба"),
    (KeyCode::A, "A", "ходьба"),
    (KeyCode::S, "S", "ходьба"),
    (KeyCode::D, "D", "ходьба"),
    (KeyCode::Up, "↑", "ходьба"),
    (KeyCode::Down, "↓", "ходьба"),
    (KeyCode::Left, "←", "ходьба"),
    (KeyCode::Right, "→", "ходьба"),
    (KeyCode::Space, "Пробел", "атака"),
    (KeyCode::E, "E", "взаимодействие"),
    (KeyCode::F, "F", "взаимодействие"),
    (KeyCode::I, "I", "инвентарь"),
    (KeyCode::K, "K", "навыки"),
    (KeyCode::U, "U", "слияние умений"),
    (KeyCode::C, "C", "персонаж"),
    (KeyCode::J, "J", "журнал"),
    (KeyCode::G, "G", "группа"),
    (KeyCode::M, "M", "карта"),
    (KeyCode::Tab, "Tab", "карта"),
    (KeyCode::H, "H", "помощь"),
    (KeyCode::L, "L", "летопись мира"),
    (KeyCode::T, "T", "чат"),
    (KeyCode::Enter, "Enter", "чат"),
    (KeyCode::KpEnter, "Num Enter", "чат"),
    (KeyCode::Slash, "/", "команды чата"),
    (KeyCode::Escape, "Esc", "меню"),
    (KeyCode::F1, "F1", "помощь"),
    (KeyCode::F5, "F5", "сохранение"),
    (KeyCode::F9, "F9", "окно администратора"),
    (KeyCode::F11, "F11", "полный экран"),
    (KeyCode::F12, "F12", "снимок экрана"),
    (KeyCode::Minus, "-", "масштаб"),
    (KeyCode::Equal, "=", "масштаб"),
    (KeyCode::KpAdd, "Num +", "масштаб"),
    (KeyCode::KpSubtract, "Num -", "масштаб"),
    (KeyCode::LeftControl, "Ctrl", "сочетания клавиш"),
    (KeyCode::RightControl, "Ctrl", "сочетания клавиш"),
    (KeyCode::LeftSuper, "Cmd", "сочетания клавиш"),
    (KeyCode::RightSuper, "Cmd", "сочетания клавиш"),
];

/// The config name of a cell.
fn cell_id(cell: usize) -> String {
    match cell {
        POTION_HEALTH => "potion_health".into(),
        POTION_MANA => "potion_mana".into(),
        c => format!("slot{}", c + 1),
    }
}

/// The name of a cell on screen.
pub fn cell_name(cell: usize) -> String {
    match cell {
        POTION_HEALTH => tr("Зелье здоровья"),
        POTION_MANA => tr("Зелье маны"),
        c => tr(&format!("Ячейка {}", c + 1)),
    }
}

fn name(b: Bind) -> String {
    match b {
        Bind::Key(k) => KEYS
            .iter()
            .find(|(x, _)| *x == k)
            .map_or_else(|| format!("{k:?}"), |(_, n)| n.to_string()),
        Bind::Mouse(m) => MOUSE
            .iter()
            .find(|(x, _, _)| *x == m)
            .map_or_else(String::new, |(_, n, _)| n.to_string()),
    }
}

fn parse(s: &str) -> Option<Bind> {
    KEYS.iter()
        .find(|(_, n)| *n == s)
        .map(|(k, _)| Bind::Key(*k))
        .or_else(|| {
            MOUSE
                .iter()
                .find(|(_, n, _)| *n == s)
                .map(|(m, _, _)| Bind::Mouse(*m))
        })
}

/// A binding as the player sees it: "1", "Num 3", "ПКМ".
pub fn label(b: Bind) -> String {
    match b {
        Bind::Mouse(m) => MOUSE
            .iter()
            .find(|(x, _, _)| *x == m)
            .map_or_else(String::new, |(_, _, l)| tr(l)),
        k => name(k),
    }
}

/// The keys of a cell out of the box: 1–9 and 0 for the first ten cells
/// (and the right mouse button for the first), Q and R for the potions.
fn defaults(cell: usize) -> [Option<Bind>; BINDS] {
    const DIGITS: [KeyCode; 10] = [
        KeyCode::Key1,
        KeyCode::Key2,
        KeyCode::Key3,
        KeyCode::Key4,
        KeyCode::Key5,
        KeyCode::Key6,
        KeyCode::Key7,
        KeyCode::Key8,
        KeyCode::Key9,
        KeyCode::Key0,
    ];
    match cell {
        0 => [
            Some(Bind::Key(KeyCode::Key1)),
            Some(Bind::Mouse(MouseButton::Right)),
        ],
        POTION_HEALTH => [Some(Bind::Key(KeyCode::Q)), None],
        POTION_MANA => [Some(Bind::Key(KeyCode::R)), None],
        c => [DIGITS.get(c).map(|k| Bind::Key(*k)), None],
    }
}

/// What a key pressed while a binding waits for one means.
pub enum Capture {
    /// nothing pressed yet
    Waiting,
    Cancel,
    /// the new binding (None: Delete or Backspace clears it)
    Set(Option<Bind>),
    /// a key the game keeps: why it cannot be bound
    Refused(String),
}

/// Reads the key or the mouse button that the player gives a binding.
pub fn capture(inp: &mut UiInput) -> Capture {
    if inp.take(KeyCode::Escape) {
        return Capture::Cancel;
    }
    if inp.take(KeyCode::Backspace) || inp.take(KeyCode::Delete) {
        return Capture::Set(None);
    }
    if inp.rclicked && !inp.used {
        inp.used = true;
        return Capture::Set(Some(Bind::Mouse(MouseButton::Right)));
    }
    if inp.mclicked {
        return Capture::Set(Some(Bind::Mouse(MouseButton::Middle)));
    }
    let Some(&k) = inp.keys.first() else {
        return Capture::Waiting;
    };
    inp.take(k);
    inp.chars.clear();
    if KEYS.iter().any(|(x, _)| *x == k) {
        return Capture::Set(Some(Bind::Key(k)));
    }
    match RESERVED.iter().find(|(x, _, _)| *x == k) {
        Some((_, n, what)) => Capture::Refused(tr(&format!("Клавиша {n} занята: {what}."))),
        None => Capture::Refused(tr("Эту клавишу назначить нельзя.")),
    }
}

/// The bindings of all cells.
#[derive(Clone, Debug, PartialEq)]
pub struct Keys {
    pub cells: Vec<[Option<Bind>; BINDS]>,
}

impl Default for Keys {
    fn default() -> Keys {
        Keys {
            cells: (0..CELLS).map(defaults).collect(),
        }
    }
}

impl Keys {
    /// The bindings of the config; cells it does not name keep the defaults.
    pub fn load(cfg: &Config) -> Keys {
        let mut k = Keys::default();
        for (i, cell) in k.cells.iter_mut().enumerate() {
            if let Some(names) = cfg.hotkeys.get(&cell_id(i)) {
                *cell = [None; BINDS];
                for (b, n) in cell.iter_mut().zip(names) {
                    *b = parse(n);
                }
            }
        }
        k
    }

    /// Writes the bindings into the config (nothing when they are the
    /// defaults).
    pub fn store(&self, cfg: &mut Config) {
        cfg.hotkeys.clear();
        let def = Keys::default();
        for (i, (cell, d)) in self.cells.iter().zip(&def.cells).enumerate() {
            if cell != d {
                let mut names: Vec<String> = cell
                    .iter()
                    .map(|b| b.map(name).unwrap_or_default())
                    .collect();
                while names.last().is_some_and(String::is_empty) {
                    names.pop();
                }
                cfg.hotkeys.insert(cell_id(i), names);
            }
        }
    }

    /// Binds a key to a cell; a binding belongs to one cell, so it leaves
    /// any other. Returns the cell it was taken from.
    pub fn set(&mut self, cell: usize, slot: usize, b: Option<Bind>) -> Option<usize> {
        let mut from = None;
        if let Some(b) = b {
            for (i, c) in self.cells.iter_mut().enumerate() {
                for (j, x) in c.iter_mut().enumerate() {
                    if *x == Some(b) && (i, j) != (cell, slot) {
                        *x = None;
                        if i != cell {
                            from = Some(i);
                        }
                    }
                }
            }
        }
        self.cells[cell][slot] = b;
        from
    }

    /// The cells whose bindings were pressed this frame (their keys are
    /// taken). Mouse buttons count only over the world, modifier keys only
    /// in the game itself.
    pub fn pressed(&self, inp: &mut UiInput, mouse: bool, modifiers: bool) -> Vec<usize> {
        let mut out = Vec::new();
        for (i, cell) in self.cells.iter().enumerate() {
            for b in cell.iter().flatten() {
                let hit = match *b {
                    Bind::Key(k) => (modifiers || !MODIFIERS.contains(&k)) && inp.take(k),
                    Bind::Mouse(MouseButton::Right) => mouse && inp.rclicked && !inp.used,
                    Bind::Mouse(MouseButton::Middle) => mouse && inp.mclicked,
                    Bind::Mouse(_) => false,
                };
                if hit && !out.contains(&i) {
                    out.push(i);
                }
            }
        }
        out
    }

    /// The first binding of a cell as shown on it ("" without one).
    pub fn label(&self, cell: usize) -> String {
        self.cells
            .get(cell)
            .and_then(|c| c.iter().flatten().next())
            .map_or_else(String::new, |b| label(*b))
    }

    /// All bindings of a cell: "1, ПКМ".
    pub fn labels(&self, cell: usize) -> String {
        self.cells.get(cell).map_or_else(String::new, |c| {
            c.iter()
                .flatten()
                .map(|b| label(*b))
                .collect::<Vec<_>>()
                .join(", ")
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bindings_round_trip_through_the_config() {
        let mut cfg = Config::default();
        let mut k = Keys::load(&cfg);
        assert_eq!(k, Keys::default());
        assert_eq!(k.label(0), "1");
        assert_eq!(k.label(9), "0");
        assert_eq!(k.label(10), "");
        // Q moves from the health potion to cell 12
        assert_eq!(
            k.set(11, 0, Some(Bind::Key(KeyCode::Q))),
            Some(POTION_HEALTH)
        );
        assert_eq!(k.label(POTION_HEALTH), "");
        k.set(2, 1, Some(Bind::Mouse(MouseButton::Middle)));
        k.set(1, 0, None);
        k.store(&mut cfg);
        assert_eq!(cfg.hotkeys["slot12"], vec!["Q"]);
        assert_eq!(cfg.hotkeys["slot3"], vec!["3", "MouseMiddle"]);
        assert_eq!(cfg.hotkeys["slot2"], Vec::<String>::new());
        assert!(!cfg.hotkeys.contains_key("slot1"));
        assert_eq!(Keys::load(&cfg), k);
        Keys::default().store(&mut cfg);
        assert!(cfg.hotkeys.is_empty());
    }

    #[test]
    fn every_bindable_key_has_a_name_and_is_free() {
        for (k, n) in KEYS {
            assert_eq!(parse(n), Some(Bind::Key(*k)));
            assert!(!RESERVED.iter().any(|(r, _, _)| r == k), "{n} is reserved");
        }
    }
}
