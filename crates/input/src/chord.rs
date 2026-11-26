//! Chord matching: key downs/ups → binding presses/releases.

use std::collections::BTreeSet;

use thiserror::Error;
use vocalis_core::gesture::{Binding, Input};

use crate::key::Key;

/// A set of keys pressed together, e.g. `Fn` or `Ctrl+Win+Shift`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chord(BTreeSet<Key>);

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ChordError {
    #[error("a shortcut needs at least one key")]
    Empty,
    #[error("a shortcut can have at most 4 keys")]
    TooManyKeys,
    #[error("\"{0}\" is not a key name")]
    UnknownKey(String),
    #[error("Esc and Enter are reserved for cancelling and stopping")]
    Reserved,
    #[error("a single letter or digit would fire while you type; add a modifier")]
    BareCharacter,
    #[error("two actions use the same shortcut")]
    Duplicate,
}

impl Chord {
    pub fn new(keys: impl IntoIterator<Item = Key>) -> Result<Self, ChordError> {
        let set: BTreeSet<Key> = keys.into_iter().collect();
        match set.len() {
            0 => return Err(ChordError::Empty),
            5.. => return Err(ChordError::TooManyKeys),
            _ => {}
        }
        if set.contains(&Key::Escape) || set.contains(&Key::Enter) {
            return Err(ChordError::Reserved);
        }
        if set
            .iter()
            .all(|k| matches!(k, Key::Char(_) | Key::Space | Key::Tab | Key::Backspace))
        {
            return Err(ChordError::BareCharacter);
        }
        Ok(Self(set))
    }

    /// Parse `"left_ctrl+space"` style settings values.
    pub fn parse(s: &str) -> Result<Self, ChordError> {
        let keys = s
            .split('+')
            .filter(|p| !p.trim().is_empty())
            .map(|p| Key::parse(p).ok_or_else(|| ChordError::UnknownKey(p.trim().to_owned())))
            .collect::<Result<Vec<_>, _>>()?;
        Self::new(keys)
    }

    pub fn keys(&self) -> impl Iterator<Item = &Key> {
        self.0.iter()
    }

    fn contains(&self, k: &Key) -> bool {
        self.0.contains(k)
    }

    fn is_subset(&self, down: &BTreeSet<Key>) -> bool {
        self.0.is_subset(down)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyEvent {
    Down(Key, u64),
    Up(Key, u64),
}

/// Tracks which keys are down and reports chord presses and releases as
/// gesture inputs. A chord fires when its last key goes down and every key
/// currently held belongs to it or to another chord that is already held.
/// That lets `Fn` → `Fn+Space` and `Ctrl+Win` → `Ctrl+Win+Shift` promote a
/// push-to-talk into hands-free, while typing `Fn+K` fires nothing new.
#[derive(Debug, Clone)]
pub struct ChordTracker {
    bindings: Vec<(Binding, Chord)>,
    down: BTreeSet<Key>,
    active: Vec<Binding>,
}

impl ChordTracker {
    pub fn new(bindings: Vec<(Binding, Chord)>) -> Result<Self, ChordError> {
        for (i, (_, a)) in bindings.iter().enumerate() {
            if bindings[i + 1..].iter().any(|(_, b)| a == b) {
                return Err(ChordError::Duplicate);
            }
        }
        Ok(Self {
            bindings,
            down: BTreeSet::new(),
            active: Vec::new(),
        })
    }

    pub fn handle(&mut self, ev: KeyEvent) -> Vec<Input> {
        let mut out = Vec::new();
        match ev {
            KeyEvent::Down(key, t) => {
                // Key repeat: the OS resends downs while held.
                if !self.down.insert(key) {
                    return out;
                }
                if key == Key::Escape {
                    out.push(Input::Escape(t));
                }
                if key == Key::Enter {
                    out.push(Input::Enter(t));
                }
                let held_by_active: BTreeSet<Key> = self
                    .bindings
                    .iter()
                    .filter(|(b, _)| self.active.contains(b))
                    .flat_map(|(_, c)| c.keys().copied())
                    .collect();
                for (binding, chord) in &self.bindings {
                    if self.active.contains(binding) || !chord.contains(&key) || !chord.is_subset(&self.down) {
                        continue;
                    }
                    let clean = self
                        .down
                        .iter()
                        .all(|k| chord.contains(k) || held_by_active.contains(k));
                    if clean {
                        self.active.push(*binding);
                        out.push(Input::Pressed(*binding, t));
                    }
                }
            }
            KeyEvent::Up(key, t) => {
                self.down.remove(&key);
                let bindings = &self.bindings;
                self.active.retain(|b| {
                    let chord = &bindings.iter().find(|(x, _)| x == b).expect("active binding exists").1;
                    if chord.contains(&key) {
                        out.push(Input::Released(*b, t));
                        false
                    } else {
                        true
                    }
                });
            }
        }
        out
    }

    /// Forget all held keys, e.g. after the screen locks or focus is lost and ups may be missed.
    pub fn reset(&mut self, t: u64) -> Vec<Input> {
        self.down.clear();
        self.active.drain(..).map(|b| Input::Released(b, t)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use Binding::*;
    use Input::*;
    use KeyEvent::{Down, Up};

    fn tracker(primary: &str, hands_free: &str) -> ChordTracker {
        ChordTracker::new(vec![
            (Primary, Chord::parse(primary).unwrap()),
            (HandsFree, Chord::parse(hands_free).unwrap()),
        ])
        .unwrap()
    }

    fn run(t: &mut ChordTracker, evs: &[KeyEvent]) -> Vec<Input> {
        evs.iter().flat_map(|e| t.handle(*e)).collect()
    }

    #[test]
    fn mac_fn_hold_and_fn_space_promotion() {
        let mut t = tracker("fn", "fn+space");
        assert_eq!(
            run(&mut t, &[Down(Key::Fn, 0), Up(Key::Fn, 900)]),
            vec![Pressed(Primary, 0), Released(Primary, 900)]
        );
        assert_eq!(
            run(
                &mut t,
                &[
                    Down(Key::Fn, 1000),
                    Down(Key::Space, 1100),
                    Up(Key::Space, 1150),
                    Up(Key::Fn, 1200)
                ]
            ),
            vec![
                Pressed(Primary, 1000),
                Pressed(HandsFree, 1100),
                Released(HandsFree, 1150),
                Released(Primary, 1200)
            ]
        );
    }

    #[test]
    fn windows_ctrl_win_then_shift() {
        let mut t = tracker("ctrl+win", "ctrl+win+shift");
        let got = run(
            &mut t,
            &[
                Down(Key::LeftCtrl, 0),
                Down(Key::LeftMeta, 10),
                Down(Key::LeftShift, 20),
            ],
        );
        assert_eq!(got, vec![Pressed(Primary, 10), Pressed(HandsFree, 20)]);
        // Pressing all three in another order fires both too.
        let mut t = tracker("ctrl+win", "ctrl+win+shift");
        let got = run(
            &mut t,
            &[Down(Key::LeftShift, 0), Down(Key::LeftCtrl, 5), Down(Key::LeftMeta, 10)],
        );
        assert_eq!(
            got,
            vec![Pressed(HandsFree, 10)],
            "Shift was held first, so only the exact 3-key chord is clean"
        );
    }

    #[test]
    fn typing_while_holding_does_not_fire() {
        let mut t = tracker("left_ctrl+space", "left_ctrl+shift+space");
        assert!(run(
            &mut t,
            &[Down(Key::Char('a'), 0), Down(Key::LeftCtrl, 5), Down(Key::Space, 10)]
        )
        .is_empty());
    }

    #[test]
    fn sides_are_distinct() {
        let mut t = ChordTracker::new(vec![(Primary, Chord::parse("right_option").unwrap())]).unwrap();
        assert!(run(&mut t, &[Down(Key::LeftAlt, 0), Up(Key::LeftAlt, 5)]).is_empty());
        assert_eq!(run(&mut t, &[Down(Key::RightAlt, 10)]), vec![Pressed(Primary, 10)]);
    }

    #[test]
    fn repeat_downs_are_ignored_and_esc_enter_pass_through() {
        let mut t = tracker("fn", "fn+space");
        assert_eq!(
            run(&mut t, &[Down(Key::Fn, 0), Down(Key::Fn, 30), Down(Key::Fn, 60)]),
            vec![Pressed(Primary, 0)]
        );
        assert_eq!(
            run(&mut t, &[Down(Key::Escape, 70), Down(Key::Enter, 80)]),
            vec![Escape(70), Enter(80)]
        );
    }

    #[test]
    fn reset_releases_everything() {
        let mut t = tracker("fn", "fn+space");
        run(&mut t, &[Down(Key::Fn, 0), Down(Key::Space, 5)]);
        assert_eq!(t.reset(9), vec![Released(Primary, 9), Released(HandsFree, 9)]);
        assert_eq!(run(&mut t, &[Down(Key::Fn, 20)]), vec![Pressed(Primary, 20)]);
    }

    #[test]
    fn chord_validation() {
        assert_eq!(Chord::parse(""), Err(ChordError::Empty));
        assert_eq!(Chord::parse("k"), Err(ChordError::BareCharacter));
        assert_eq!(Chord::parse("space"), Err(ChordError::BareCharacter));
        assert_eq!(Chord::parse("ctrl+esc"), Err(ChordError::Reserved));
        assert_eq!(Chord::parse("ctrl+alt+shift+cmd+k"), Err(ChordError::TooManyKeys));
        assert_eq!(
            Chord::parse("ctrl+banana"),
            Err(ChordError::UnknownKey("banana".into()))
        );
        assert!(Chord::parse("f5").is_ok());
        assert!(Chord::parse("mouse4").is_ok());
        let c = Chord::parse("fn").unwrap();
        assert_eq!(
            ChordTracker::new(vec![(Primary, c.clone()), (HandsFree, c)]).unwrap_err(),
            ChordError::Duplicate
        );
    }

    mod props {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            /// Presses and releases per binding alternate, whatever keys are mashed.
            #[test]
            fn presses_and_releases_alternate(evs in proptest::collection::vec((0usize..6, any::<bool>()), 0..80)) {
                let keys = [Key::Fn, Key::Space, Key::LeftCtrl, Key::LeftShift, Key::Char('k'), Key::Escape];
                let mut t = tracker("fn", "fn+space");
                let mut held = [false; 2];
                for (i, (k, down)) in evs.into_iter().enumerate() {
                    let ev = if down { Down(keys[k], i as u64) } else { Up(keys[k], i as u64) };
                    for input in t.handle(ev) {
                        match input {
                            Pressed(b, _) => { let i = b as usize; prop_assert!(!held[i]); held[i] = true; }
                            Released(b, _) => { let i = b as usize; prop_assert!(held[i]); held[i] = false; }
                            _ => {}
                        }
                    }
                }
            }
        }
    }
}
