#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StatusKind {
    Strength,
    Vulnerable,
    Weak,
    Ritual,
    Enrage,
    CurlUp,
    StrengthDown,
}

/// One creature's statuses. Typed fields (not a map): the Phase 1 set is
/// closed, and exhaustive struct access keeps rule code obvious.
#[derive(Clone, Default, Debug, PartialEq, Eq)]
pub struct Statuses {
    /// Adds to each attack hit. Can go negative.
    pub strength: i32,
    /// Turns of taking ×1.5 attack damage.
    pub vulnerable: u32,
    /// Turns of dealing ×0.75 attack damage.
    pub weak: u32,
    /// Gains this much Strength at end of own turn…
    pub ritual: u32,
    /// …except the turn Ritual was applied (StS Cultist hits 6, 9, 12…).
    pub ritual_fresh: bool,
    /// Gains this much Strength whenever the *player* plays a Skill.
    pub enrage: u32,
    /// One-shot: gain this much Block the first time an attack deals ≥1 damage.
    pub curl_up: Option<u32>,
    /// Loses this much Strength at end of own turn, then clears (Surge of Rage).
    pub strength_down: u32,
}

impl Statuses {
    /// End-of-own-turn duration tick. Returns (kind, remaining) per decrement,
    /// in fixed order, so the combat engine can emit events.
    pub fn tick_durations(&mut self) -> Vec<(StatusKind, u32)> {
        let mut out = Vec::new();
        if self.vulnerable > 0 {
            self.vulnerable -= 1;
            out.push((StatusKind::Vulnerable, self.vulnerable));
        }
        if self.weak > 0 {
            self.weak -= 1;
            out.push((StatusKind::Weak, self.weak));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_all_zero() {
        let s = Statuses::default();
        assert_eq!(s.strength, 0);
        assert_eq!(s.vulnerable, 0);
        assert_eq!(s.weak, 0);
        assert_eq!(s.ritual, 0);
        assert!(!s.ritual_fresh);
        assert_eq!(s.enrage, 0);
        assert_eq!(s.curl_up, None);
        assert_eq!(s.strength_down, 0);
    }

    #[test]
    fn durations_stack_additively() {
        let mut s = Statuses::default();
        s.vulnerable += 2;
        s.vulnerable += 1;
        assert_eq!(s.vulnerable, 3);
    }

    #[test]
    fn tick_durations_decrements_and_reports() {
        let mut s = Statuses {
            vulnerable: 2,
            weak: 1,
            ..Default::default()
        };
        let ticked = s.tick_durations();
        assert_eq!(s.vulnerable, 1);
        assert_eq!(s.weak, 0);
        assert_eq!(
            ticked,
            vec![(StatusKind::Vulnerable, 1), (StatusKind::Weak, 0)]
        );

        let ticked = s.tick_durations();
        assert_eq!(s.vulnerable, 0);
        assert_eq!(ticked, vec![(StatusKind::Vulnerable, 0)]);

        assert_eq!(s.tick_durations(), vec![]);
    }
}
