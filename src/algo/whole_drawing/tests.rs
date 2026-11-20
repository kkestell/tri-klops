use super::*;
use crate::algo::geometry::ShapeRules;
use crate::algo::mean_color;
use image::Rgb;
use rand::SeedableRng;

#[test]
fn colors_fit_the_visible_part_of_each_layer() {
    let background = [23, 45, 67];
    let mut reference = RgbImage::from_pixel(10, 10, Rgb(background));
    let mut triangles = vec![
        Triangle {
            vertices: [[0, 0], [10, 0], [0, 10]],
            color: [230, 40, 80],
        },
        Triangle {
            vertices: [[2, 0], [10, 0], [2, 9]],
            color: [2, 78, 170],
        },
    ];
    for triangle in &triangles {
        triangle.paint(&mut reference);
    }
    let expected = triangles.clone();
    // A duplicate underneath the first layer is fully covered.
    triangles.insert(0, triangles[0]);
    for triangle in &mut triangles {
        triangle.color = [0; 3];
    }
    let mut drawing = Drawing {
        background,
        triangles,
        error: 0,
    };
    // One uncovered pixel differs from the background.
    reference.get_pixel_mut(9, 9)[2] += 10;
    let target_squared = reference
        .as_raw()
        .iter()
        .map(|&v| i64::from(v).pow(2))
        .sum();
    drawing.fit(&reference, target_squared, &mut [0; 100]);
    assert_eq!(drawing.triangles, expected);
    assert_eq!(drawing.error, 100);
}

#[test]
fn local_refinement_can_move_a_single_grid_coordinate() {
    let params = AlgorithmParams {
        grid_spacing: 4,
        ..Default::default()
    };
    let triangle = Triangle {
        vertices: [[4, 4], [20, 4], [4, 20]],
        color: [0; 3],
    };
    let moved = nudge(
        triangle.vertices,
        [32; 2],
        params.grid_spacing,
        &mut SmallRng::seed_from_u64(42),
    );
    let changes: Vec<_> = moved
        .iter()
        .flatten()
        .zip(triangle.vertices.iter().flatten())
        .filter(|(a, b)| a != b)
        .map(|(a, b)| (a - b).abs())
        .collect();
    assert_eq!(changes, [4]);
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn later_generations_refine_a_drawing_at_its_limit() {
    let reference = RgbImage::from_fn(24, 24, |x, y| Rgb([(10 * x) as u8, (10 * y) as u8, 80]));
    let background = RgbImage::from_pixel(24, 24, Rgb(mean_color(&reference)));
    let run = |generations| {
        let params = AlgorithmParams {
            num_generations: generations,
            num_triangles: 1,
            image_size: 24,
            population_size: 12,
            num_selected: 3,
            grid_spacing: 2,
            degeneracy_threshold: Some(10.0),
            ..Default::default()
        };
        let progress = Mutex::new(Progress::default());
        let triangles = pollster::block_on(evolve(
            &params,
            &reference,
            background.clone(),
            &progress,
            &mut SmallRng::seed_from_u64(42),
            &mut async |_| {},
        ));
        let progress = progress.into_inner().unwrap();
        assert_eq!(triangles.len(), 1);
        assert_eq!(triangles.len(), progress.triangle_count);
        let triangle = triangles[0];
        assert!(triangle.vertices.iter().flatten().all(|v| v % 2 == 0));
        assert!(ShapeRules::new(&params).allows(triangle.vertices));

        let mut painted = background.clone();
        triangle.paint(&mut painted);
        assert_eq!(progress.canvas.as_deref(), Some(&painted));
        let error: i64 = painted
            .as_raw()
            .iter()
            .zip(reference.as_raw())
            .map(|(&p, &t)| (i64::from(p) - i64::from(t)).pow(2))
            .sum();
        assert_eq!(
            progress.current_fitness,
            crate::algo::raster::fitness(error, reference.dimensions())
        );
        error
    };
    assert!(run(24) < run(1));
}
