use std::collections::BTreeMap;

use super::{
    DecisionOutcome, PhaseExecution, Simulation, SimulationError, SimulationTurnReport, TurnPhase,
};

impl Simulation {
    /// Advances exactly one global turn. All settlements finish a phase before
    /// any settlement starts the next. Within a phase, IDs are ascending.
    /// Preflight failures retain queued decisions and do not advance the counter.
    /// Individual invalid decisions are reported and do not abort the turn.
    pub fn step(&mut self) -> Result<SimulationTurnReport, SimulationError> {
        let turn = self
            .current_turn
            .checked_add(1)
            .ok_or(SimulationError::TurnOverflow)?;
        let ids: Vec<_> = self
            .world
            .settlements()
            .map(|settlement| settlement.id())
            .collect();
        for &id in &ids {
            self.world
                .validate_settlement_turn(id)
                .map_err(|error| SimulationError::World {
                    settlement_id: id,
                    error,
                })?;
        }

        let mut execution = Vec::new();
        let mut decisions = Vec::new();
        let mut pending = std::mem::take(&mut self.pending_decisions);
        for &id in &ids {
            execution.push(PhaseExecution {
                phase: TurnPhase::Decisions,
                settlement_id: id,
            });
            for decision in pending.remove(&id).unwrap_or_default() {
                let result = self.apply_decision(id, &decision);
                decisions.push(DecisionOutcome {
                    settlement_id: id,
                    decision,
                    result,
                });
            }
        }

        let mut prepared = Vec::with_capacity(ids.len());
        for &id in &ids {
            execution.push(PhaseExecution {
                phase: TurnPhase::Preparation,
                settlement_id: id,
            });
            let (settlement, map, territory) = self
                .world
                .settlement_turn_parts(id)
                .expect("registered settlement");
            prepared.push((
                id,
                settlement.prepare_turn(map, &mut self.finder, territory),
            ));
        }

        let mut allocated = Vec::with_capacity(prepared.len());
        for (id, state) in prepared {
            execution.push(PhaseExecution {
                phase: TurnPhase::Allocation,
                settlement_id: id,
            });
            let (settlement, _, _) = self
                .world
                .settlement_turn_parts(id)
                .expect("registered settlement");
            allocated.push((id, settlement.allocate_turn(state)));
        }

        let mut consumed = Vec::with_capacity(allocated.len());
        for (id, state) in allocated {
            execution.push(PhaseExecution {
                phase: TurnPhase::Consumption,
                settlement_id: id,
            });
            let (settlement, _, _) = self
                .world
                .settlement_turn_parts(id)
                .expect("registered settlement");
            consumed.push((id, settlement.consume_turn(state)));
        }

        for (id, state) in &consumed {
            let id = *id;
            execution.push(PhaseExecution {
                phase: TurnPhase::Construction,
                settlement_id: id,
            });
            let (settlement, _, _) = self
                .world
                .settlement_turn_parts(id)
                .expect("registered settlement");
            settlement.progress_construction(state);
        }

        let mut produced = Vec::with_capacity(consumed.len());
        for (id, state) in consumed {
            execution.push(PhaseExecution {
                phase: TurnPhase::Production,
                settlement_id: id,
            });
            let (settlement, _, _) = self
                .world
                .settlement_turn_parts(id)
                .expect("registered settlement");
            produced.push((id, settlement.produce_turn(state)));
        }

        let mut settlements = BTreeMap::new();
        for (id, state) in produced {
            execution.push(PhaseExecution {
                phase: TurnPhase::EconomicUpdates,
                settlement_id: id,
            });
            let (settlement, map, territory) = self
                .world
                .settlement_turn_parts(id)
                .expect("registered settlement");
            settlements.insert(
                id,
                settlement.finish_turn(state, map, &mut self.finder, territory),
            );
        }
        self.current_turn = turn;
        Ok(SimulationTurnReport {
            turn,
            decisions,
            settlements,
            execution,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::SettlementId;
    use crate::world::{Map, Province, World, terrain::area::Area};

    #[test]
    fn all_settlements_are_validated_before_decisions_or_economic_mutations() {
        use super::super::SettlementDecision;
        use crate::buildings::definition::BuildingDefinition;
        use crate::economy::ResourceType::Wood;
        use crate::ids::ProjectId;
        use crate::settlement::{Settlement, projects::ProjectCost};
        use crate::world::{Position, WorldError};
        use std::collections::HashMap;
        use std::sync::Arc;

        let mut world = World::new(
            Map::new(2, 1, vec![Area::new(0.5, 0.5); 2], vec![10, 20]),
            vec![Province::new(10), Province::new(20)],
        );
        let definition = Arc::new(BuildingDefinition {
            id: "house".into(),
            name: "House".into(),
            tier: 1,
            max_workers: 0,
            behaviors: vec![],
            construction_cost: ProjectCost::default(),
        });
        for (province, id, x) in [(10, 1, 0), (20, 2, 1)] {
            let mut settlement = Settlement::new(SettlementId(id), 100, 0, Arc::default());
            settlement.deposit(Wood, 5);
            world.add_settlement(province, settlement).unwrap();
            world
                .queue_construction(
                    SettlementId(id),
                    ProjectId(7),
                    Position { x, y: 0 },
                    definition.clone(),
                    ProjectCost {
                        needed_resources: HashMap::from([(Wood, 1)]),
                        turn_cost: 2,
                    },
                )
                .unwrap();
        }
        world.map.territory[1] = 10;
        let mut simulation = Simulation::new(world).unwrap();
        simulation
            .queue_decision(
                SettlementId(1),
                SettlementDecision::CancelProject(ProjectId(7)),
            )
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
                assert_eq!(settlement.inventory().amount(Wood), 5);
                assert_eq!(
                    settlement.active_projects()[0].remaining_cost().turn_cost,
                    2
                );
            }
        }
        simulation.world.map.territory[1] = 20;
        let report = simulation.step().unwrap();
        assert_eq!(report.decisions.len(), 1);
        assert_eq!(report.decisions[0].result, Ok(()));
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
                crate::settlement::Settlement::new(
                    SettlementId(1),
                    100,
                    0,
                    std::sync::Arc::default(),
                ),
            )
            .unwrap();
        simulation
            .queue_decision(
                SettlementId(1),
                super::super::SettlementDecision::CancelProject(crate::ids::ProjectId(7)),
            )
            .unwrap();
        simulation.current_turn = u64::MAX;
        assert_eq!(
            simulation.step().unwrap_err(),
            SimulationError::TurnOverflow
        );
        assert_eq!(simulation.current_turn(), u64::MAX);
        assert_eq!(simulation.pending_decisions[&SettlementId(1)].len(), 1);
    }
}
