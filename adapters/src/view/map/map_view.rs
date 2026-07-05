use ::rand::RngExt;
use ::rand::prelude::ThreadRng;
use core::domain::{
    geography::{biome::Biome, elevation::Elevation},
    world_data::{map::Map, world::World},
};
use macroquad::prelude::*;
pub struct MapView {
    pub map_texture: Texture2D,
}
impl MapView {
    pub fn generate_from_world(world: &World) -> MapView {
        let width: usize = world.map.width;
        let height: usize = world.map.height;
        let mut map_image: Image = Image::gen_image_color(width as u16, height as u16, BLANK);
        let mut rng: ThreadRng = ::rand::rng();
        let image_data: &mut [[u8; 4]] = map_image.get_image_data_mut();
        let ocean_level: f32 = Elevation::OCEAN_LEVEL;

        for y in 0..height {
            for x in 0..width {
                let base_color: Color =
                    MapView::determine_color(&world.map.terrain[y * width + x].determine_biome());
                let noise: f32 = rng.random_range(-0.04..=0.04);
                let lightning_multiplier: f32 =
                    MapView::calculate_light(&world.map, x, y, width, height, ocean_level);
                let final_color: Color = Color::new(
                    (base_color.r * lightning_multiplier + noise).clamp(0.0, 1.0),
                    (base_color.g * lightning_multiplier + noise).clamp(0.0, 1.0),
                    (base_color.b * lightning_multiplier + noise).clamp(0.0, 1.0),
                    1.0,
                );
                image_data[y * width + x] = final_color.into();
            }
        }

        MapView::extract_map_borders(&mut map_image, width, height);
        MapView::extract_province_borders(world, &mut map_image, width, height);

        let map_texture: Texture2D = Texture2D::from_image(&map_image);
        map_texture.set_filter(FilterMode::Nearest);

        MapView { map_texture }
    }

    fn calculate_light(
        map: &Map,
        x: usize,
        y: usize,
        width: usize,
        height: usize,
        ocean_level: f32,
    ) -> f32 {
        if map.terrain[y * width + x].elevation_val < ocean_level {
            return 1.0;
        }

        let x_left: usize = x.saturating_sub(1);
        let x_right: usize = (x + 1).min(width - 1);
        let y_up: usize = y.saturating_sub(1);
        let y_down: usize = (y + 1).min(height - 1);

        let h_left: f32 = map.terrain[y * width + x_left].elevation_val;
        let h_right: f32 = map.terrain[y * width + x_right].elevation_val;
        let h_up: f32 = map.terrain[y_up * width + x].elevation_val;
        let h_down: f32 = map.terrain[y_down * width + x].elevation_val;

        let scale: f32 = 25.0;
        let dz_dx: f32 = (h_right - h_left) * scale;
        let dz_dy: f32 = (h_down - h_up) * scale;

        let n_len: f32 = (dz_dx * dz_dx + dz_dy * dz_dy + 1.0).sqrt();
        let nx: f32 = -dz_dx / n_len;
        let ny: f32 = -dz_dy / n_len;
        let nz: f32 = 1.0 / n_len;

        let lx: f32 = -0.577;
        let ly: f32 = -0.577;
        let lz: f32 = 0.577;

        let dot: f32 = (nx * lx + ny * ly + nz * lz).max(0.0);

        let ambient: f32 = 0.4;
        let diffuse: f32 = 0.8;

        ambient + (diffuse * dot)
    }

    fn determine_color(biome: &Biome) -> Color {
        match biome {
            Biome::Ocean => color_u8!(68, 75, 115, 255),
            Biome::Desert => color_u8!(198, 185, 155, 255),
            Biome::Grassland => color_u8!(148, 173, 110, 255),
            Biome::Forest => color_u8!(95, 140, 90, 255),
            Biome::Rainforest => color_u8!(55, 105, 75, 255),
            Biome::Hills => color_u8!(140, 130, 115, 255),
            Biome::Taiga => color_u8!(105, 135, 120, 255),
            Biome::Tundra => color_u8!(180, 180, 175, 255),
            Biome::Snow => color_u8!(240, 240, 245, 255),
        }
    }

    fn extract_map_borders(map_image: &mut Image, width: usize, height: usize) {
        for x in 0..width {
            map_image.set_pixel(x as u32, 0, BLACK);
            map_image.set_pixel(x as u32, height as u32 - 1, BLACK);
        }
        for y in 0..height {
            map_image.set_pixel(0, y as u32, BLACK);
            map_image.set_pixel(width as u32 - 1, y as u32, BLACK);
        }
    }

    fn extract_province_borders(world: &World, map_image: &mut Image, width: usize, height: usize) {
        for y in 1..height - 1 {
            for x in 1..width - 1 {
                let province_id: usize = world.map.territory[y * width + x];

                if province_id != world.map.territory[(y - 1) * width + x]
                    || province_id != world.map.territory[y * width + x - 1]
                    || province_id != world.map.territory[(y + 1) * width + x]
                    || province_id != world.map.territory[y * width + x + 1]
                {
                    map_image.set_pixel(x as u32, y as u32, BLACK);
                }
            }
        }
    }
}
