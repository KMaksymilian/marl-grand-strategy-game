use macroquad::prelude::*;
pub struct MapCamera {
    pub speed: f32,
    pub camera: Camera2D,
}
impl MapCamera {
    pub fn new(speed: f32) -> MapCamera {
        let camera: Camera2D =
            Camera2D::from_display_rect(Rect::new(0.0, 0.0, screen_width(), screen_height()));
        MapCamera { speed, camera }
    }

    pub fn begin(&self) {
        set_camera(&self.camera);
    }

    pub fn end(&self) {
        set_default_camera();
    }

    pub fn update(&mut self) {
        if is_key_down(KeyCode::W) {
            self.camera.target.y -= self.speed;
        }
        if is_key_down(KeyCode::S) {
            self.camera.target.y += self.speed;
        }
        if is_key_down(KeyCode::A) {
            self.camera.target.x -= self.speed;
        }
        if is_key_down(KeyCode::D) {
            self.camera.target.x += self.speed;
        }

        let (_, wheel) = mouse_wheel();
        if wheel != 0.0 {
            self.camera.zoom *= 1.0 + wheel * 0.1;
        }
    }
}
