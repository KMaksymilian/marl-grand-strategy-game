pub fn material(id: &str) -> simulation_core::economy::ResourceType {
    simulation_core::economy::ResourceCatalog::standard()
        .resolve(id)
        .expect("test material must exist in the bundled resource catalog")
}
