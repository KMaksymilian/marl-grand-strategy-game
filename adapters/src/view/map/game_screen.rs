use crate::view::map::{map_camera::MapCamera, map_texture::MapTexture};
use macroquad::prelude::*;
pub struct GameScreen {
    map_texture: MapTexture,
    camera: MapCamera,
}
impl GameScreen {
    pub fn new(map_texture: MapTexture, camera: MapCamera) -> GameScreen {
        GameScreen {
            map_texture,
            camera,
        }
    }
    pub fn run(&mut self) {
        self.camera.update();
        self.camera.begin();
        draw_texture(&self.map_texture.inner, 0.0, 0.0, WHITE);
        self.camera.end();
    }
}
