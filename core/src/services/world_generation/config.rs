pub struct WorldDimensions {
    pub width: usize,
    pub height: usize,
}
impl Default for WorldDimensions {
    fn default() -> WorldDimensions {
        WorldDimensions {
            width: 2000,
            height: 1500,
        }
    }
}

pub struct VoronoiConfig {
    pub province_count: usize,
    pub iteration_count: usize,
}
impl Default for VoronoiConfig {
    fn default() -> VoronoiConfig {
        VoronoiConfig {
            province_count: 500,
            iteration_count: 3,
        }
    }
}

pub struct NoiseConfig {
    pub elevation_scale: f64,
    pub moisture_scale: f64,
    pub min_max: (f64, f64),
}
impl Default for NoiseConfig {
    fn default() -> NoiseConfig {
        NoiseConfig {
            elevation_scale: 200.0,
            moisture_scale: 600.0,
            min_max: (0.0, 1.0),
        }
    }
}

pub struct WarpConfig {
    pub warp_scale: f64,
    pub warp_intensity: f64,
}
impl Default for WarpConfig {
    fn default() -> WarpConfig {
        WarpConfig {
            warp_scale: 50.0,
            warp_intensity: 12.5,
        }
    }
}

pub struct SeedConfig {
    pub points_seed_option: Option<u64>,
    pub borders_seed_option: Option<u32>,
    pub elevation_seed_option: Option<u32>,
    pub moisture_seed_option: Option<u32>,
}
impl Default for SeedConfig {
    fn default() -> SeedConfig {
        SeedConfig {
            points_seed_option: Some(67),
            borders_seed_option: Some(69),
            elevation_seed_option: Some(420),
            moisture_seed_option: Some(2137),
        }
    }
}

pub struct ElevationTuningFineConfig {
    pub distance_multiplyer: f64,
    pub dropoff_powi: i32,
    pub dropoff_multiplyer: f64,
    pub final_val_addition: f64,
}
impl Default for ElevationTuningFineConfig {
    fn default() -> ElevationTuningFineConfig {
        ElevationTuningFineConfig {
            distance_multiplyer: 0.8,
            dropoff_powi: 4,
            dropoff_multiplyer: 1.5,
            final_val_addition: 0.25,
        }
    }
}
