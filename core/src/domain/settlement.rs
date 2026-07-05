use super::resource::ResourceInventory;

#[derive(Debug)]
pub struct Settlement {
    pub id: u32,
    pub resource_inventory: ResourceInventory,
}

impl Settlement {
    pub fn new(id: u32, initial_capacity: f32) -> Self {
        Self {
            id,
            resource_inventory: ResourceInventory::new(initial_capacity),
        }
    }

    //todo
    pub fn process_turn(&mut self) {
        panic!("Not implemented yet");

    }
}