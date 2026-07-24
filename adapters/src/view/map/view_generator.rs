use core::domain::world_data::world::World;

use crate::view::map::{game_screen::GameScreen, map_camera::MapCamera, map_view::MapView};

pub struct ViewGenerator;

impl ViewGenerator {
    pub fn generate(world: &World) -> GameScreen {
        let map_view: MapView = MapView::generate_from_world(world);
        let map_camera: MapCamera =
            MapCamera::new(0.5, world.map.width as f32, world.map.height as f32);
        GameScreen::new(map_view, map_camera)
    }
}
