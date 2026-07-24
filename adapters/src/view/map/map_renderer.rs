use super::map_view::MapView;
use macroquad::prelude::*;
pub struct MapRenderer;
impl MapRenderer {
    pub fn draw(view: &MapView) {
        draw_texture(&view.map_texture, 0.0, 0.0, WHITE);
    }
}
