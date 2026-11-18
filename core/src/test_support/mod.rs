//! Test fixtures and suites that advance isolated domain state.
pub(crate) fn material(id: &str) -> crate::economy::ResourceType {
    crate::economy::ResourceCatalog::standard()
        .resolve(id)
        .expect("test material must exist in the bundled resource catalog")
}

mod building_productivity_tests;
mod decision_validation_tests;
mod project_cancellation_tests;
mod settlement_environment_tests;
mod simulation_tests;
mod territory_construction_tests;
mod territory_trade_integration_tests;
mod trade_network;
