# World Generation

`WorldBuilder` in `core/src/services/world_generation/` generates a flat grid map
with terrain and provinces. It is independent of Macroquad. Individual source
files and methods are indexed in the [code map](code_map.md).

## Configuration

Generation parameters are Rust structs in `world_generation/config.rs`, separate
from the runtime economy parser in `core/src/configuration.rs`.

| Parameter type | Current defaults |
| --- | --- |
| `WorldDimensions` | Width 2000, height 1500 |
| `VoronoiConfig` | 500 provinces, 3 relaxation passes |
| `NoiseConfig` | Elevation scale 200, moisture scale 600, range 0..1 |
| `WarpConfig` | Scale 50, intensity 12.5 |
| `SeedConfig` | Point seed 67, borders 69, elevation 420, moisture 2137 |
| `ElevationTuningFineConfig` | Radial multiplier 0.8, exponent 4, dropoff multiplier 1.5, addition 0.25 |

`generate_default()` uses these defaults; `generate()` accepts explicit parameters.
Missing optional seeds use random values. `NoiseManager` uses four-octave Perlin
FBM and supplies noise samples, coordinate warp and island elevation falloff.

## Pipeline

1. Allocate terrain and territory buffers in `WorldBuilder::new()`.
2. Fill elevation with warped island noise and moisture with warped noise.
3. Generate unique province seeds, empty province records and a border warp field.
4. Repeatedly assign tiles to the nearest warped seed and recompute centroids.
5. Apply `edge_alignment()`: if a province includes an ocean tile, make that whole
   province ocean.
6. Apply `ocean_set()`: normalize ocean elevation to zero and moisture to one.
7. Return `World` with map, provinces and an empty caravan collection.

Terrain filling and tile assignment use Rayon. `WorldBuilderFunctions` holds
seed generation, Voronoi assignment and centroid helpers.

## Terrain data

`Map` stores width/height, `Vec<Area>` terrain and a flat province-index territory
buffer. `Province` stores its ID and tile coordinates. `Area` stores elevation
and moisture values and classifies them through geography enums.

| Classification | Thresholds |
| --- | --- |
| Elevation | Ocean below 0.2; lowland below 0.4; plains below 0.6; highland below 0.8; mountain otherwise |
| Moisture | Dry below 0.4; normal below 0.5; humid below 0.6; wet otherwise |

`Biome::determine()` combines both categories into ocean, desert, grassland,
forest, rainforest, hills, taiga, tundra or snow. Pathfinding reads raw elevation,
not the biome category, when computing movement costs.

## Assumptions

Generation assumes valid, nonzero dimensions and a feasible province count.
Unique-point generation cannot terminate when more points are requested than
available tiles; centroid calculation requires nonempty provinces. `Map::new()`
does not validate its input vector lengths. Callers must currently enforce these
constraints. Rendering adds unseeded cosmetic noise, so identical domain seeds do
not guarantee identical texture pixels.
