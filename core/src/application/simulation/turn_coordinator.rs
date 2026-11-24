//! Executes the phases of one global turn.
use std::collections::BTreeMap;

use super::{
    NationDecision, PhaseExecution, SettlementDecision, SimulationError, SimulationTurnReport,
    TurnPhase,
};
use crate::application::commands::nation::handler::handle_nation_decision;
use crate::application::commands::settlement::{CommandDispatch, handler::handle_decision};
use crate::domain::ids::{NationId, SettlementId};
use crate::domain::nation::report::NationTurnReport;
use crate::domain::nation::taxation::{MaterialTotals, NationTaxReport};
use crate::domain::world::{World, pathfinding::path_finder::PathFinder};

/// Completes each phase for every settlement in ascending ID order.
/// Preflight errors preserve the world and the decision queue.
pub(super) fn run_turn(
    world: &mut World,
    finder: &mut PathFinder,
    pending_decisions: &mut BTreeMap<SettlementId, Vec<SettlementDecision>>,
    pending_nation_decisions: &mut BTreeMap<NationId, Vec<NationDecision>>,
    current_turn: u64,
) -> Result<SimulationTurnReport, SimulationError> {
    let turn = current_turn
        .checked_add(1)
        .ok_or(SimulationError::TurnOverflow)?;
    let ids: Vec<_> = world
        .settlements()
        .map(|settlement| settlement.id())
        .collect();
    for &id in &ids {
        world
            .validate_settlement_turn(id)
            .map_err(|error| SimulationError::World {
                settlement_id: id,
                error,
            })?;
    }

    // Nations decide first, so settlements decide under the new policy and with
    // this turn's subsidies. The queue is a BTreeMap: ascending nation ID. The
    // `expect` cannot fire: `queue_nation_decision` rejects unknown nations and
    // `Simulation::transfer_settlement` drops the queue of an extinct nation.
    let mut nation_decisions = Vec::new();
    for (_, queued) in std::mem::take(pending_nation_decisions) {
        for decision in queued {
            nation_decisions.push(
                handle_nation_decision(world, &decision)
                    .expect("queued decisions belong to registered nations"),
            );
        }
    }

    // Upkeep of active decrees, right after the decisions (a decree revoked
    // above pays nothing). A decree that cannot be paid is revoked (cooldown)
    // and has no effect this turn.
    let mut upkeep = world.pay_decree_upkeep();

    let mut execution = Vec::new();
    let mut decisions = Vec::new();
    let mut pending = std::mem::take(pending_decisions);
    for &id in &ids {
        execution.push(PhaseExecution {
            phase: TurnPhase::Decisions,
            settlement_id: id,
        });
        for decision in pending.remove(&id).unwrap_or_default() {
            decisions.push(
                handle_decision(CommandDispatch::new(world), &decision)
                    .expect("queued decisions belong to registered settlements"),
            );
        }
    }

    world.begin_trade_turn();
    let mut prepared = Vec::with_capacity(ids.len());
    for &id in &ids {
        execution.push(PhaseExecution {
            phase: TurnPhase::Preparation,
            settlement_id: id,
        });
        prepared.push((id, world.prepare_settlement_turn(id, finder, turn)));
    }

    let mut allocated = Vec::with_capacity(prepared.len());
    for (id, state) in prepared {
        execution.push(PhaseExecution {
            phase: TurnPhase::Allocation,
            settlement_id: id,
        });
        let settlement = world.settlement_mut(id).expect("registered settlement");
        allocated.push((id, settlement.allocate_turn(state)));
    }

    let mut consumed = Vec::with_capacity(allocated.len());
    for (id, state) in allocated {
        execution.push(PhaseExecution {
            phase: TurnPhase::Consumption,
            settlement_id: id,
        });
        let settlement = world.settlement_mut(id).expect("registered settlement");
        consumed.push((id, settlement.consume_turn(state)));
    }

    for (id, state) in &consumed {
        let id = *id;
        execution.push(PhaseExecution {
            phase: TurnPhase::Construction,
            settlement_id: id,
        });
        let settlement = world.settlement_mut(id).expect("registered settlement");
        settlement.progress_construction(state);
    }

    let mut produced = Vec::with_capacity(consumed.len());
    for (id, state) in consumed {
        execution.push(PhaseExecution {
            phase: TurnPhase::Production,
            settlement_id: id,
        });
        let settlement = world.settlement_mut(id).expect("registered settlement");
        produced.push((id, settlement.produce_turn(state)));
    }

    // Taxation: the owner's rate × this turn's gross output of every
    // settlement, before storage. Settlements of one nation fill its stock in
    // ascending ID, which decides who meets a full capacity.
    let mut taxes: BTreeMap<NationId, NationTaxReport> = BTreeMap::new();
    let mut taxed = Vec::with_capacity(produced.len());
    for (id, mut state) in produced {
        execution.push(PhaseExecution {
            phase: TurnPhase::Taxation,
            settlement_id: id,
        });
        let collection = world
            .collect_taxes(id, &mut state)
            .expect("every registered settlement has an owner");
        let nation = taxes.entry(collection.report.nation_id).or_default();
        add_totals(&mut nation.stock_received, &collection.stock_received);
        add_totals(&mut nation.stock_lost, &collection.stock_lost);
        nation.settlements.insert(id, collection.report);
        taxed.push((id, state));
    }

    let mut settlements = BTreeMap::new();
    for (id, state) in taxed {
        execution.push(PhaseExecution {
            phase: TurnPhase::EconomicUpdates,
            settlement_id: id,
        });
        let (settlement, mut environment) = world
            .settlement_turn_parts(id, finder)
            .expect("registered settlement");
        let mut report = settlement.finish_turn(state, &mut environment);
        report.taxation = taxes
            .values()
            .find_map(|nation| nation.settlements.get(&id))
            .cloned();
        settlements.insert(id, report);
    }
    let locations = world.capital_trade_locations();
    for (sender, routes) in world.finish_trade_turn(&locations, finder) {
        settlements
            .get_mut(&sender)
            .expect("every caravan sender has a report")
            .trade
            .record_departure_routes(routes);
    }
    // National timers count completed turns, so they tick once, after every
    // phase; the nation reports show the state at the end of the turn.
    world.advance_nations_turn();
    let nations = world
        .nations()
        .iter()
        .map(|nation| {
            let id = nation.id();
            let settlements = world.nations().settlements_of(id);
            let taxes = taxes.remove(&id).unwrap_or_default();
            let upkeep = upkeep.remove(&id).unwrap_or_default();
            (
                id,
                NationTurnReport::closing(nation, settlements, taxes, upkeep),
            )
        })
        .collect();
    Ok(SimulationTurnReport {
        turn,
        nation_decisions,
        nations,
        decisions,
        settlements,
        execution,
    })
}

fn add_totals(totals: &mut MaterialTotals, added: &MaterialTotals) {
    for (resource, &amount) in added {
        *totals.entry(resource.clone()).or_default() += amount;
    }
}

#[cfg(test)]
mod tests {
    use super::super::Simulation;
    use super::*;
    use crate::domain::world::{Map, Province, World, terrain::area::Area};

    #[test]
    fn all_settlements_are_validated_before_decisions_or_economic_mutations() {
        use super::super::{SettlementAction, SettlementDecision};
        use crate::domain::buildings::definition::BuildingDefinition;
        use crate::domain::ids::ProjectId;
        use crate::domain::settlement::{Settlement, projects::ProjectCost};
        use crate::domain::world::{Position, WorldError};
        use std::collections::HashMap;
        use std::sync::Arc;

        let mut world = World::new(
            Map::new(3, 1, vec![Area::new(0.5, 0.5); 3], vec![10, 20, 20]),
            vec![Province::new(10), Province::new(20)],
        );
        let wood = world.resource_catalog().resolve("wood").unwrap();
        let definition = Arc::new(
            BuildingDefinition::new("house", "House", 1, 0, vec![], ProjectCost::default())
                .unwrap(),
        );
        for (province, id, x, capital_x) in [(10, 1, 0, 0), (20, 2, 1, 2)] {
            let mut settlement = Settlement::new(SettlementId(id), 100, 0, Arc::default());
            settlement.deposit(wood.clone(), 5);
            world
                .add_settlement_at(province, Position { x: capital_x, y: 0 }, settlement)
                .unwrap();
            world
                .queue_construction(
                    SettlementId(id),
                    ProjectId(7),
                    Position { x, y: 0 },
                    definition.clone(),
                    ProjectCost {
                        needed_resources: HashMap::from([(wood.clone(), 1)]),
                        turn_cost: 2,
                    },
                )
                .unwrap();
        }
        world
            .set_tile_province(Position { x: 1, y: 0 }, 10)
            .unwrap();
        let mut simulation = Simulation::new(world).unwrap();
        simulation
            .queue_decision(SettlementDecision::new(
                SettlementId(1),
                vec![SettlementAction::CancelProject {
                    project_id: ProjectId(7),
                }],
            ))
            .unwrap();
        for _ in 0..2 {
            assert!(matches!(
                simulation.step(),
                Err(SimulationError::World {
                    settlement_id: SettlementId(2),
                    error: WorldError::SettlementOutsideProvince { .. }
                })
            ));
            assert_eq!(simulation.current_turn(), 0);
            assert_eq!(simulation.pending_decisions[&SettlementId(1)].len(), 1);
            for settlement in simulation.settlements() {
                assert_eq!(settlement.inventory().amount(wood.clone()), 5);
                assert_eq!(
                    settlement.active_projects()[0].remaining_cost().turn_cost,
                    2
                );
            }
        }
        simulation
            .world
            .set_tile_province(Position { x: 1, y: 0 }, 20)
            .unwrap();
        let report = simulation.step().unwrap();
        assert_eq!(report.decisions.len(), 1);
        assert!(report.decisions[0].results[0].is_applied());
        assert!(
            simulation
                .world()
                .settlement(SettlementId(1))
                .unwrap()
                .active_projects()
                .is_empty()
        );
        assert_eq!(
            simulation
                .world()
                .settlement(SettlementId(2))
                .unwrap()
                .active_projects()[0]
                .remaining_cost()
                .turn_cost,
            1
        );
        assert_eq!(simulation.current_turn(), 1);
    }

    #[test]
    fn exhausted_counter_rejects_step_without_mutating_state_or_decisions() {
        let world = World::new(
            Map::new(1, 1, vec![Area::new(0.5, 0.5)], vec![0]),
            vec![Province::new(0)],
        );
        let mut simulation = Simulation::new(world).unwrap();
        simulation
            .add_settlement(
                0,
                crate::domain::settlement::Settlement::new(
                    SettlementId(1),
                    100,
                    0,
                    std::sync::Arc::default(),
                ),
            )
            .unwrap();
        simulation
            .queue_decision(super::super::SettlementDecision::new(
                SettlementId(1),
                vec![super::super::SettlementAction::CancelProject {
                    project_id: crate::domain::ids::ProjectId(7),
                }],
            ))
            .unwrap();
        simulation.world.set_completed_turn(u64::MAX);
        assert_eq!(
            simulation.step().unwrap_err(),
            SimulationError::TurnOverflow
        );
        assert_eq!(simulation.current_turn(), u64::MAX);
        assert_eq!(simulation.pending_decisions[&SettlementId(1)].len(), 1);
    }
}
