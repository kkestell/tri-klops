use std::{path::Path, sync::Mutex, time::Instant};
use triklops::algo::{AlgorithmParams, Progress, SearchMode, run_algorithm};

fn measure(name: &str, seed: u64, mut params: AlgorithmParams, directory: &Path) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("examples/{name}.jpg"));
    let reference = image::open(path)
        .unwrap()
        .resize(
            params.image_size,
            params.image_size,
            image::imageops::FilterType::Lanczos3,
        )
        .into_rgb8();
    let output = directory.join(format!("{name}-{seed}-{}.svg", params.num_triangles));
    params.seed = Some(seed);
    let progress = Mutex::new(Progress::default());
    let start = Instant::now();
    run_algorithm(&params, &reference, &output, &progress).unwrap();
    let elapsed = start.elapsed().as_secs_f64();
    let canvas = progress.into_inner().unwrap().canvas.unwrap();
    let sse: i64 = canvas
        .as_raw()
        .iter()
        .zip(reference.as_raw())
        .map(|(&a, &b)| (i64::from(a) - i64::from(b)).pow(2))
        .sum();
    println!(
        "{name},{seed},{elapsed:.6},{:.4}",
        sse as f64 / reference.as_raw().len() as f64
    );
}

fn main() {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("target/approximation-bench");
    std::fs::create_dir_all(&directory).unwrap();
    println!("image,seed,seconds,rgb_mse");
    if std::env::args().any(|arg| arg == "--defaults") {
        measure("castle", 42, AlgorithmParams::default(), &directory);
    } else {
        let params = AlgorithmParams {
            num_triangles: 32,
            image_size: 256,
            num_generations: 12,
            population_size: 48,
            num_selected: 12,
            ..AlgorithmParams::for_mode(SearchMode::Greedy)
        };
        for seed in [7, 42, 101] {
            for name in ["castle", "kanagawa", "tri-klops", "zardoz"] {
                measure(name, seed, params.clone(), &directory);
            }
        }
    }
}
