use core::{
    domain::world_data::world::World,
    services::algorithms::{a_star::AStar, dijkstra_flood::DijkstraFlood, path_finder::PathFinder},
};

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
    pub a_star_clicked_provinces: Vec<(usize, usize)>,
    pub dijkstra_flood_clicked_provinces: Vec<(usize, usize)>,
    pub a_star_path: Vec<(usize, usize)>,
    pub dijkstra_flood_path: Vec<(usize, usize)>,
}

impl GameScreen {
    pub fn new(map_texture_data: MapTextureData, camera: MapCamera) -> GameScreen {
        GameScreen {
            map_texture_data,
            camera_type: CameraType::Geography,
            camera,
            a_star_clicked_provinces: Vec::new(),
            dijkstra_flood_clicked_provinces: Vec::new(),
            a_star_path: Vec::new(),
            dijkstra_flood_path: Vec::new(),
        }
    }

    pub fn run(&mut self, world: &World, path_finder: &mut PathFinder) {
        self.camera.update();
        if let Some(camera_type) = self.camera.select_camera_type() {
            self.camera_type = camera_type;
        };

        let active_texture: &Texture2D = &self.map_texture_data.textures[0];
        let width: u32 = active_texture.width() as u32;
        let height: u32 = active_texture.height() as u32;

        if is_mouse_button_pressed(MouseButton::Left) {
            let screen_pos: Vec2 = Vec2::from(mouse_position());
            let world_pos: Vec2 = self.camera.camera.screen_to_world(screen_pos);

            if world_pos.x >= 0.0 && world_pos.y >= 0.0 {
                let tile_x = world_pos.x.floor() as u32;
                let tile_y = world_pos.y.floor() as u32;

                if tile_x < width && tile_y < height {
                    self.a_star_clicked_provinces
                        .push((tile_x as usize, tile_y as usize));

                    if self.a_star_clicked_provinces.len() > 2 {
                        self.a_star_clicked_provinces.remove(0);
                    }

                    if self.a_star_clicked_provinces.len() == 2 {
                        if let Some(points) = AStar::a_star(
                            &world.map,
                            path_finder,
                            self.a_star_clicked_provinces[0],
                            self.a_star_clicked_provinces[1],
                        ) {
                            self.a_star_path = points;
                        } else {
                            self.a_star_path.clear();
                        }
                    }
                }
            }
        }

        if is_mouse_button_pressed(MouseButton::Right) {
            let screen_pos: Vec2 = Vec2::from(mouse_position());
            let world_pos: Vec2 = self.camera.camera.screen_to_world(screen_pos);

            if world_pos.x >= 0.0 && world_pos.y >= 0.0 {
                let tile_x = world_pos.x.floor() as u32;
                let tile_y = world_pos.y.floor() as u32;

                if tile_x < width && tile_y < height {
                    self.dijkstra_flood_clicked_provinces
                        .push((tile_x as usize, tile_y as usize));

                    if self.dijkstra_flood_clicked_provinces.len() > 1 {
                        self.dijkstra_flood_clicked_provinces.remove(0);
                    }

                    if self.dijkstra_flood_clicked_provinces.len() == 1 {
                        self.dijkstra_flood_path = DijkstraFlood::dijkstra_flood(
                            &world.map,
                            path_finder,
                            self.dijkstra_flood_clicked_provinces[0],
                            1000,
                        );
                    }
                }
            }
        }

        self.camera.begin();

        draw_texture(
            &self.map_texture_data.textures[self.camera_type.index()],
            0.0,
            0.0,
            WHITE,
        );

        for &idx in &self.dijkstra_flood_path {
            draw_rectangle(idx.0 as f32, idx.1 as f32, 1.0, 1.0, PURPLE);
        }

        for &idx in &self.a_star_path {
            draw_rectangle(idx.0 as f32, idx.1 as f32, 1.0, 1.0, RED);
        }

        for &(tx, ty) in &self.a_star_clicked_provinces {
            draw_rectangle(tx as f32, ty as f32, 1.0, 1.0, BLUE);
        }

        for &(tx, ty) in &self.dijkstra_flood_clicked_provinces {
            draw_rectangle(tx as f32, ty as f32, 1.0, 1.0, GREEN);
        }

        self.camera.end();
    }
}
