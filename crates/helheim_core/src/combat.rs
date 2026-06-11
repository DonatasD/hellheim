/// StS damage pipeline, integer math (truncating division == floor here):
/// strength adds to base, Weak multiplies ×0.75, Vulnerable ×1.5.
pub fn attack_damage(base: u32, strength: i32, attacker_weak: bool, target_vulnerable: bool) -> u32 {
    let mut d = (base as i32 + strength).max(0) as u32;
    if attacker_weak {
        d = d * 3 / 4;
    }
    if target_vulnerable {
        d = d * 3 / 2;
    }
    d
}

#[must_use]
pub struct DamageOutcome {
    pub blocked: u32,
    pub hp_lost: u32,
}

/// Apply damage to block first, overflow to HP. Mutates in place.
pub fn soak(block: &mut u32, hp: &mut u32, amount: u32) -> DamageOutcome {
    let blocked = amount.min(*block);
    *block -= blocked;
    let hp_lost = (amount - blocked).min(*hp);
    *hp -= hp_lost;
    DamageOutcome { blocked, hp_lost }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_damage_passes_through() {
        assert_eq!(attack_damage(6, 0, false, false), 6);
    }

    #[test]
    fn strength_adds_before_multipliers() {
        assert_eq!(attack_damage(6, 3, false, false), 9);
    }

    #[test]
    fn negative_strength_clamps_at_zero() {
        assert_eq!(attack_damage(2, -5, false, false), 0);
    }

    #[test]
    fn weak_is_three_quarters_floored() {
        assert_eq!(attack_damage(6, 0, true, false), 4); // floor(4.5)
        assert_eq!(attack_damage(7, 0, true, false), 5); // floor(5.25)
        assert_eq!(attack_damage(8, 0, true, false), 6); // exact
    }

    #[test]
    fn vulnerable_is_one_and_a_half_floored() {
        assert_eq!(attack_damage(6, 0, false, true), 9); // exact
        assert_eq!(attack_damage(5, 0, false, true), 7); // floor(7.5)
    }

    #[test]
    fn pipeline_order_is_strength_then_weak_then_vulnerable() {
        // Distinguishing case: base 10 → weak 7 → vuln 10.
        // Vuln first would give: 10→15→11. Assert 10.
        assert_eq!(attack_damage(10, 0, true, true), 10);
    }

    #[test]
    fn soak_consumes_block_before_hp() {
        let mut block = 5;
        let mut hp = 80;
        let out = soak(&mut block, &mut hp, 8);
        assert_eq!((block, hp), (0, 77));
        assert_eq!((out.blocked, out.hp_lost), (5, 3));
    }

    #[test]
    fn soak_leaves_leftover_block() {
        let mut block = 10;
        let mut hp = 80;
        let out = soak(&mut block, &mut hp, 4);
        assert_eq!((block, hp), (6, 80));
        assert_eq!((out.blocked, out.hp_lost), (4, 0));
    }

    #[test]
    fn soak_does_not_underflow_hp() {
        let mut block = 0;
        let mut hp = 3;
        let out = soak(&mut block, &mut hp, 99);
        assert_eq!(hp, 0);
        assert_eq!(out.hp_lost, 3);
    }
}
