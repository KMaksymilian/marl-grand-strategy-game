use crate::view::map::{map_camera::MapCamera, map_texture::MapTextureData};
use macroquad::prelude::*;

pub enum CameraType {
    Geography,
    Eleavation,
}
impl CameraType {
    fn index(&self) -> usize {
        match self {
            CameraType::Geography => 0,
            CameraType::Eleavation => 1,
        }
    }
}

pub struct GameScreen {
    pub map_texture_data: MapTextureData,
    pub camera_type: CameraType,
    pub camera: MapCamera,
}
impl GameScreen {
    pub fn new(map_texture_data: MapTextureData, camera: MapCamera) -> GameScreen {
        let camera_type: CameraType = CameraType::Geography;
        GameScreen {
            map_texture_data,
            camera_type,
            camera,
        }
    }
    pub fn run(&mut self) {
        self.camera.update();
        self.camera.begin();
        draw_texture(
            &self.map_texture_data.textures[self.camera_type.index()],
            0.0,
            0.0,
            WHITE,
        );
        self.camera.end();
    }
}
