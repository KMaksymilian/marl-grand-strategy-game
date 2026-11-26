use simulation_core::{
    world::pathfinding::{a_star::AStar, dijkstra_flood::DijkstraFlood, path_finder::PathFinder},
    world::{Point, Position, World},
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
    pub a_star_clicked_provinces: Vec<Point>,
    pub dijkstra_flood_clicked_provinces: Vec<Point>,
    pub a_star_path: Vec<Point>,
    pub dijkstra_flood_path: Vec<Point>,
    pub deposits_visible: bool,
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
            deposits_visible: true,
        }
    }

    fn cursor_tile(&self, width: u32, height: u32) -> Option<Point> {
        let position = self
            .camera
            .camera
            .screen_to_world(Vec2::from(mouse_position()));
        if !(position.x >= 0.0 && position.y >= 0.0) {
            return None;
        }
        let x = position.x.floor() as u32;
        let y = position.y.floor() as u32;
        (x < width && y < height).then_some(Point(x as usize, y as usize))
    }

    pub fn run(&mut self, world: &World, path_finder: &mut PathFinder) {
        self.camera.update();
        if is_key_pressed(KeyCode::O) {
            self.deposits_visible = !self.deposits_visible;
        }
        if let Some(camera_type) = self.camera.select_camera_type() {
            self.camera_type = camera_type;
        };

        let active_texture: &Texture2D = &self.map_texture_data.textures[0];
        let width: u32 = active_texture.width() as u32;
        let height: u32 = active_texture.height() as u32;

        if is_mouse_button_pressed(MouseButton::Left)
            && let Some(point) = self.cursor_tile(width, height)
        {
            self.a_star_clicked_provinces.push(point);

            if self.a_star_clicked_provinces.len() > 2 {
                self.a_star_clicked_provinces.remove(0);
            }

            if self.a_star_clicked_provinces.len() == 2 {
                if let Some(points) = AStar::a_star(
                    world.map(),
                    path_finder,
                    (
                        self.a_star_clicked_provinces[0].0,
                        self.a_star_clicked_provinces[0].1,
                    ),
                    (
                        self.a_star_clicked_provinces[1].0,
                        self.a_star_clicked_provinces[1].1,
                    ),
                ) {
                    self.a_star_path = points;
                } else {
                    self.a_star_path.clear();
                }
            }
        }

        if is_mouse_button_pressed(MouseButton::Right)
            && let Some(point) = self.cursor_tile(width, height)
        {
            self.dijkstra_flood_clicked_provinces.push(point);

            if self.dijkstra_flood_clicked_provinces.len() > 1 {
                self.dijkstra_flood_clicked_provinces.remove(0);
            }

            if self.dijkstra_flood_clicked_provinces.len() == 1 {
                self.dijkstra_flood_path = DijkstraFlood::dijkstra_flood(
                    world.map(),
                    path_finder,
                    (
                        self.dijkstra_flood_clicked_provinces[0].0,
                        self.dijkstra_flood_clicked_provinces[0].1,
                    ),
                    1000,
                );
            }
        }

        self.camera.begin();

        draw_texture(
            &self.map_texture_data.textures[self.camera_type.index()],
            0.0,
            0.0,
            WHITE,
        );

        if self.deposits_visible {
            for (position, resource) in world.map().deposits() {
                let color = match resource.as_str() {
                    "Coal" => DARKGRAY,
                    "IronOre" => ORANGE,
                    "CopperOre" => PINK,
                    "TinOre" => SKYBLUE,
                    _ => GOLD,
                };
                draw_rectangle(position.x as f32, position.y as f32, 1.0, 1.0, color);
            }
        }

        for &idx in &self.dijkstra_flood_path {
            draw_rectangle(idx.0 as f32, idx.1 as f32, 1.0, 1.0, PURPLE);
        }

        for &idx in &self.a_star_path {
            draw_rectangle(idx.0 as f32, idx.1 as f32, 1.0, 1.0, RED);
        }

        for &point in &self.a_star_clicked_provinces {
            draw_rectangle(point.0 as f32, point.1 as f32, 1.0, 1.0, BLUE);
        }

        for &point in &self.dijkstra_flood_clicked_provinces {
            draw_rectangle(point.0 as f32, point.1 as f32, 1.0, 1.0, GREEN);
        }

        self.camera.end();

        let layer = if self.deposits_visible { "on" } else { "off" };
        draw_rectangle(8.0, 8.0, 440.0, 76.0, Color::new(0.0, 0.0, 0.0, 0.8));
        draw_text(
            format!("Ore deposits {layer}  [O]"),
            18.0,
            30.0,
            20.0,
            WHITE,
        );
        if self.deposits_visible {
            for (index, (name, color)) in [
                ("Coal", DARKGRAY),
                ("Iron", ORANGE),
                ("Copper", PINK),
                ("Tin", SKYBLUE),
            ]
            .into_iter()
            .enumerate()
            {
                let x = 18.0 + index as f32 * 100.0;
                draw_rectangle(x, 40.0, 10.0, 10.0, color);
                draw_text(name, x + 16.0, 51.0, 18.0, WHITE);
            }
        }
        if let Some(point) = self.cursor_tile(width, height) {
            let position = Position {
                x: point.0 as i32,
                y: point.1 as i32,
            };
            if let Ok(tile) = world.inspect_tile(position) {
                let deposit = tile
                    .deposit
                    .as_ref()
                    .and_then(|resource| world.resource_catalog().get(resource));
                let shore = if tile.coastal { "Coastal land" } else { "" };
                let ore = deposit.map(|definition| definition.name()).unwrap_or("");
                draw_text(
                    format!("({}, {})  {shore}  {ore}", point.0, point.1),
                    18.0,
                    73.0,
                    18.0,
                    WHITE,
                );
            }
        }
    }
}
