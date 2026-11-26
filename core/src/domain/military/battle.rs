//! Battle arithmetic (gameplay specification §11.3 and §11.4), independent of
//! the world: the strength of each side, the advantages, the walls and the
//! losses. The world decides who fights, draws the random variance and
//! applies the results.
//!
//! Strength of a side: Σ over its units of
//! `stat × living soldiers / soldiers per unit × advantage × terrain × morale / 100`,
//! for the defenders also × walls; then × the random variance (±10%). A unit
//! with fewer soldiers after earlier battles counts proportionally less.
use super::{Unit, UnitCatalog};

/// Which statistic a side fights with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stat {
    Attack,
    Defense,
}

/// Multiplier of a unit type against the enemy side: the advantage multiplier
/// (×1.5) weighted by the share of enemy soldiers of the types it beats, so a
/// mixed enemy army gives a proportional bonus. 1 without an advantage.
pub fn advantage(catalog: &UnitCatalog, unit: &Unit, enemies: &[Unit]) -> f32 {
    let total: u32 = enemies.iter().map(|enemy| enemy.soldiers).sum();
    if total == 0 {
        return 1.0;
    }
    let beaten: u32 = enemies
        .iter()
        .filter(|enemy| catalog.has_advantage(&unit.unit_type, &enemy.unit_type))
        .map(|enemy| enemy.soldiers)
        .sum();
    let bonus = catalog.rules().advantage_multiplier - 1.0;
    1.0 + bonus * beaten as f32 / total as f32
}

/// The strength of one side before walls and the random variance.
pub fn side_strength(
    catalog: &UnitCatalog,
    units: &[Unit],
    enemies: &[Unit],
    stat: Stat,
    terrain: f32,
) -> f32 {
    let full = catalog.rules().soldiers_per_unit as f32;
    let max_morale = 100.0;
    units
        .iter()
        .map(|unit| {
            let Some(definition) = catalog.get(&unit.unit_type) else {
                return 0.0;
            };
            let value = match stat {
                Stat::Attack => definition.attack(),
                Stat::Defense => definition.defense(),
            } as f32;
            value * unit.soldiers as f32 / full
                * advantage(catalog, unit, enemies)
                * terrain
                * unit.morale as f32
                / max_morale
        })
        .sum()
}

/// Multiplier of the defenders' strength from walls (§11.4): 1 plus the wall
/// bonus (0.25, 0.5 or 1.0 for the wall levels, plus decree points), minus
/// each attacking siege unit's reduction (0.25), at least 1.
pub fn wall_multiplier(catalog: &UnitCatalog, wall_bonus: f32, attackers: &[Unit]) -> f32 {
    let reduction: f32 = attackers
        .iter()
        .filter_map(|unit| catalog.get(&unit.unit_type))
        .map(|definition| definition.wall_bonus_reduction())
        .sum();
    1.0 + (wall_bonus - reduction).max(0.0)
}

/// Removes a share of a unit's soldiers, rounded up for the loser and down
/// for the winner; returns how many fell.
pub fn lose_soldiers(unit: &mut Unit, share: f32, round_up: bool) -> u32 {
    // The f32 share is rounded to 6 decimals first: 0.3 as f32 is
    // 0.30000001…, which would turn 15 of 50 into 16 when rounding up.
    let share = (f64::from(share.clamp(0.0, 1.0)) * 1e6).round() / 1e6;
    let exact = f64::from(unit.soldiers) * share;
    let lost = if round_up {
        (exact - 1e-9).ceil()
    } else {
        (exact + 1e-9).floor()
    };
    let lost = (lost.max(0.0) as u32).min(unit.soldiers);
    unit.soldiers -= lost;
    lost
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::ids::SettlementId;
    use crate::domain::military::UnitTypeId;

    fn unit(unit_type: &str, soldiers: u32, morale: u32) -> Unit {
        Unit {
            unit_type: UnitTypeId::new(unit_type).unwrap(),
            soldiers,
            morale,
            raised_in: SettlementId(1),
        }
    }

    #[test]
    fn strength_scales_with_soldiers_and_morale() {
        let catalog = UnitCatalog::standard();
        let enemy = [unit("levy", 50, 100)];
        let full = side_strength(
            catalog,
            &[unit("archers", 50, 100)],
            &enemy,
            Stat::Attack,
            1.0,
        );
        assert_eq!(full, 3.0);
        let half = side_strength(
            catalog,
            &[unit("archers", 25, 100)],
            &enemy,
            Stat::Attack,
            1.0,
        );
        assert_eq!(half, 1.5);
        let tired = side_strength(
            catalog,
            &[unit("archers", 50, 50)],
            &enemy,
            Stat::Attack,
            1.0,
        );
        assert_eq!(tired, 1.5);
        let defense = side_strength(
            catalog,
            &[unit("spearmen", 50, 100)],
            &enemy,
            Stat::Defense,
            1.0,
        );
        assert_eq!(defense, 3.0);
    }

    #[test]
    fn advantages_follow_the_triangle_weighted_by_the_enemy_mix() {
        let catalog = UnitCatalog::standard();
        let spearmen = unit("spearmen", 50, 100);
        assert_eq!(
            advantage(catalog, &spearmen, &[unit("cavalry", 50, 100)]),
            1.5
        );
        assert_eq!(
            advantage(catalog, &spearmen, &[unit("archers", 50, 100)]),
            1.0
        );
        let mixed = [unit("cavalry", 50, 100), unit("levy", 50, 100)];
        assert_eq!(advantage(catalog, &spearmen, &mixed), 1.25);
        assert_eq!(advantage(catalog, &spearmen, &[]), 1.0);
    }

    #[test]
    fn siege_engines_lower_the_wall_bonus_down_to_nothing() {
        let catalog = UnitCatalog::standard();
        assert_eq!(wall_multiplier(catalog, 0.5, &[]), 1.5);
        let engines = [unit("siege_engines", 50, 100), unit("levy", 50, 100)];
        assert_eq!(wall_multiplier(catalog, 0.5, &engines), 1.25);
        let three = [
            unit("siege_engines", 50, 100),
            unit("siege_engines", 50, 100),
            unit("siege_engines", 50, 100),
        ];
        assert_eq!(wall_multiplier(catalog, 0.5, &three), 1.0);
    }

    #[test]
    fn the_loser_rounds_losses_up_and_the_winner_down() {
        let mut loser = unit("levy", 50, 100);
        assert_eq!(lose_soldiers(&mut loser, 0.3, true), 15);
        let mut odd = unit("levy", 7, 100);
        assert_eq!(lose_soldiers(&mut odd, 0.3, true), 3, "2.1 rounded up");
        let mut winner = unit("levy", 7, 100);
        assert_eq!(
            lose_soldiers(&mut winner, 0.3, false),
            2,
            "2.1 rounded down"
        );
        let mut last = unit("levy", 1, 100);
        assert_eq!(lose_soldiers(&mut last, 0.3, true), 1);
        assert_eq!(last.soldiers, 0);
    }
}
