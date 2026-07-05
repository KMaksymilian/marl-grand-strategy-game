use macroquad::prelude::*;
pub struct MapCamera {
    pub speed: f32,
    pub camera: Camera2D,
}
impl MapCamera {
    pub fn new(speed: f32, map_width: f32, map_height: f32) -> MapCamera {
        let camera: Camera2D =
            Camera2D::from_display_rect(Rect::new(0.0, 0.0, map_width, map_height));
        MapCamera { speed, camera }
    }

    pub fn begin(&self) {
        set_camera(&self.camera);
    }

    pub fn end(&self) {
        set_default_camera();
    }

    pub fn update(&mut self) {
        let current_speed: f32 = self.speed / self.camera.zoom.x.abs();

        if is_key_down(KeyCode::W) {
            self.camera.target.y += current_speed;
        }
        if is_key_down(KeyCode::S) {
            self.camera.target.y -= current_speed;
        }
        if is_key_down(KeyCode::A) {
            self.camera.target.x -= current_speed;
        }
        if is_key_down(KeyCode::D) {
            self.camera.target.x += current_speed;
        }

        let (_, wheel) = mouse_wheel();
        if wheel != 0.0 {
            let zoom_factor: f32 = 1.1f32;
            if wheel.signum() > 0.0 {
                self.camera.zoom *= zoom_factor;
            } else {
                self.camera.zoom *= 1.0 / zoom_factor;
            }
        }
    }
}
