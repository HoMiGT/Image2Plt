use image::{DynamicImage, GrayImage, Luma};
use image2plt::config::AppConfig;
use image2plt::converter::{Point, Polyline, plt_encoder, preprocessor, vectorize};

// 读取实际输出的多边形缓冲区，按 HP-GL 的 PM0/PM1/PM2 语义分隔子轮廓。
fn filled_polygons(plt: &str) -> Vec<Vec<Polyline>> {
    let mut position = Point::new(0.0, 0.0);
    let mut in_polygon = false;
    let mut solid = false;
    let mut buffer: Vec<Polyline> = Vec::new();
    let mut fills = Vec::new();
    for command in plt.split(';').map(str::trim).filter(|s| !s.is_empty()) {
        match command {
            "PM0" => {
                assert!(!in_polygon);
                in_polygon = true;
                buffer = vec![vec![position]];
            }
            "PM1" => {
                assert!(in_polygon);
                buffer.push(Vec::new());
            }
            "PM2" => {
                assert!(in_polygon);
                in_polygon = false;
            }
            "FT1" => {
                assert!(!in_polygon);
                solid = true;
            }
            "FP0" => {
                assert!(!in_polygon && solid);
                assert!(!buffer.is_empty());
                for ring in &buffer {
                    assert!(ring.len() >= 4);
                    assert_eq!(ring.first(), ring.last());
                }
                fills.push(buffer.clone());
            }
            _ => {
                if let Some(coords) = command
                    .strip_prefix("PU")
                    .or_else(|| command.strip_prefix("PD"))
                {
                    if coords.is_empty() {
                        continue;
                    }
                    let values: Vec<f64> = coords.split(',').map(|n| n.parse().unwrap()).collect();
                    assert_eq!(values.len() % 2, 0);
                    for pair in values.chunks_exact(2) {
                        position = Point::new(pair[0], pair[1]);
                        if in_polygon {
                            buffer.last_mut().unwrap().push(position);
                        }
                    }
                }
            }
        }
    }
    assert!(!in_polygon);
    fills
}

fn is_filled(fills: &[Vec<Polyline>], point: Point) -> bool {
    fills.iter().any(|rings| {
        let crossings = rings
            .iter()
            .flat_map(|ring| ring.windows(2))
            .filter(|edge| {
                let (a, b) = (edge[0], edge[1]);
                (a.y > point.y) != (b.y > point.y)
                    && point.x < a.x + (point.y - a.y) * (b.x - a.x) / (b.y - a.y)
            })
            .count();
        crossings % 2 == 1
    })
}

#[test]
fn test_filled_regions_match_black_pixels_and_preserve_holes() {
    let cases: &[&[&[u8]]] = &[
        &[&[1, 1], &[1, 0]],                   // 用户的 L 形。
        &[&[1, 1, 1], &[1, 0, 1], &[1, 1, 1]], // 白色孔洞。
        &[&[1, 0], &[0, 1]],                   // 对角接触。
        &[&[1, 0, 1], &[1, 0, 1]],             // 分离的黑块。
        &[
            &[1, 1, 1, 1, 1],
            &[1, 0, 0, 0, 1],
            &[1, 0, 1, 0, 1], // 孔洞中的黑色岛。
            &[1, 0, 0, 0, 1],
            &[1, 1, 1, 1, 1],
        ],
        &[&[0, 0], &[0, 0]], // 全白，不填充。
    ];
    for &mask in cases {
        let (width, height) = (mask[0].len() as u32, mask.len() as u32);
        let img = DynamicImage::ImageLuma8(GrayImage::from_fn(width, height, |x, y| {
            Luma([if mask[y as usize][x as usize] == 1 {
                0
            } else {
                255
            }])
        }));
        for flip_y in [false, true] {
            let mut config = AppConfig::default();
            config.plt.output_width_mm = width as f64;
            config.plt.flip_y = flip_y;
            let binary = preprocessor::preprocess(&img, &config.vectorize);
            let paths = vectorize::extract_polylines(&binary, &config.vectorize);
            let plt = plt_encoder::encode_plt(&paths, width, height, &config.plt);
            let fills = filled_polygons(&plt);
            if !paths.is_empty() {
                assert_eq!(fills.len(), 1, "轮廓必须合并填充，不能分别涂黑每个孔洞");
                assert_eq!(fills[0].len(), paths.len());
            }
            // 每个像素内部采样 9 个点，检查凹角和空白没有被误填。
            for y in 0..height {
                for x in 0..width {
                    for dy in [0.25, 0.5, 0.75] {
                        for dx in [0.25, 0.5, 0.75] {
                            let py = y as f64 + dy;
                            let point = Point::new(
                                (x as f64 + dx) * 40.0,
                                (if flip_y { height as f64 - py } else { py }) * 40.0,
                            );
                            assert_eq!(
                                is_filled(&fills, point),
                                mask[y as usize][x as usize] == 1,
                                "填充应与黑白区域一致: ({x}, {y}), flip_y={flip_y}"
                            );
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn test_outline_mode_and_legacy_config() {
    let mut config = AppConfig::default();
    let legacy = toml::to_string(&config)
        .unwrap()
        .replace("fill = true\n", "");
    assert!(toml::from_str::<AppConfig>(&legacy).unwrap().plt.fill);
    config.plt.fill = false;
    config.plt.output_width_mm = 0.0;
    config.plt.flip_y = false;
    let saved = toml::to_string(&config).unwrap();
    assert!(!toml::from_str::<AppConfig>(&saved).unwrap().plt.fill);
    let img = GrayImage::from_raw(2, 2, vec![255, 255, 255, 0]).unwrap();
    let paths = vectorize::extract_polylines(&img, &config.vectorize);
    let plt = plt_encoder::encode_plt(&paths, 2, 2, &config.plt);
    assert_eq!(
        plt,
        "IN;\nSP1;\nPU0,0;\nPD2,0,2,1,1,1,1,2,0,2,0,0;\nPU;\nSP0;\n"
    );
}

#[test]
fn test_open_paths_stay_outside_polygon_buffer() {
    let mut config = AppConfig::default();
    config.plt.output_width_mm = 0.0;
    config.plt.flip_y = false;
    let img = GrayImage::from_pixel(2, 2, Luma([255]));
    let mut paths = vectorize::extract_polylines(&img, &config.vectorize);
    paths.push(vec![Point::new(5.0, 5.0), Point::new(6.0, 6.0)]);
    paths.push(Vec::new());
    let plt = plt_encoder::encode_plt(&paths, 10, 10, &config.plt);
    let fills = filled_polygons(&plt);
    assert_eq!(fills.len(), 1);
    assert_eq!(fills[0].len(), 1);
    assert!(plt.contains("FP0;\nPU5,5;\nPD6,6;"));
    assert!(!plt_encoder::encode_plt(&paths[1..], 10, 10, &config.plt).contains("PM0"));
}
