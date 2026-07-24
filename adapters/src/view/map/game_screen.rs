use crate::view::map::{map_camera::MapCamera, map_renderer::MapRenderer, map_view::MapView};
pub struct GameScreen {
    map_view: MapView,
    camera: MapCamera,
}
impl GameScreen {
    pub fn new(map_view: MapView, camera: MapCamera) -> GameScreen {
        GameScreen { map_view, camera }
    }
    pub fn run(&mut self) {
        self.camera.update();
        self.camera.begin();
        MapRenderer::draw(&self.map_view);
        self.camera.end();
    }
}
