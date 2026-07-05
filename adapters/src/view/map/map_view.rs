use core::domain::{geography::biome::Biome, world_data::world::World};
use macroquad::prelude::*;
pub struct MapView {
    pub map_texture: Texture2D,
}
impl MapView {
    pub fn generate_from_world(world: &World) -> MapView {
        let width: usize = world.map.width;
        let height: usize = world.map.height;

        let mut map_image: Image = Image::gen_image_color(width as u16, height as u16, BLANK);

        for y in 0..height {
            for x in 0..width {
                let color: Color = MapView::determine_color(&world.map.terrain[y][x].biome);
                map_image.set_pixel(x as u32, y as u32, color);
            }
        }

        MapView::extract_map_borders(&mut map_image, width, height);
        MapView::extract_province_borders(world, &mut map_image, width, height);

        let map_texture: Texture2D = Texture2D::from_image(&map_image);
        map_texture.set_filter(FilterMode::Nearest);

        MapView { map_texture }
    }

    fn determine_color(biome: &Biome) -> Color {
        // match biome {
        //     Biome::Ocean => color_u8!(21, 21, 38, 255),
        //     Biome::Desert => color_u8!(91, 87, 78, 255),
        //     Biome::Grassland => color_u8!(77, 83, 67, 255),
        //     Biome::Forest => color_u8!(66, 80, 64, 255),
        //     Biome::Rainforest => color_u8!(61, 73, 66, 255),
        //     Biome::Hills => color_u8!(77, 80, 73, 255),
        //     Biome::Taiga => color_u8!(80, 83, 73, 255),
        //     Biome::Tundra => color_u8!(87, 87, 73, 255),
        //     Biome::Snow => color_u8!(97, 97, 97, 255),
        // }
        match biome {
            Biome::Ocean => color_u8!(40, 90, 200, 255),        // żywy niebieski
            Biome::Desert => color_u8!(240, 210, 120, 255),     // piaskowy, jasny
            Biome::Grassland => color_u8!(80, 200, 90, 255),    // soczysta zieleń
            Biome::Forest => color_u8!(30, 140, 60, 255),       // ciemniejsza, ale nadal żywa zieleń
            Biome::Rainforest => color_u8!(20, 110, 70, 255),   // głęboka tropikalna zieleń
            Biome::Hills => color_u8!(170, 140, 90, 255),       // ziemisty brąz
            Biome::Taiga => color_u8!(90, 160, 140, 255),       // chłodna zieleń/niebieski mix
            Biome::Tundra => color_u8!(190, 200, 210, 255),     // zimny szaro-błękit
            Biome::Snow => color_u8!(245, 245, 250, 255),       // prawie biały, lekko niebieski
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
                let province_id: usize = world.map.territory[y][x];

                if province_id != world.map.territory[y - 1][x]
                    || province_id != world.map.territory[y][x - 1]
                    || province_id != world.map.territory[y + 1][x]
                    || province_id != world.map.territory[y][x + 1]
                {
                    map_image.set_pixel(x as u32, y as u32, BLACK);
                }
            }
        }
    }
}
