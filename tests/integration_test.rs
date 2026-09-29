use image::{ImageBuffer, Rgb};
use image2plt::config::AppConfig;
use image2plt::converter::{Point, convert_image_to_plt, preprocessor, vectorize};
use std::fs;

#[test]
fn test_full_pipeline_conversion() {
    // 1. 创建临时测试目录
    let test_dir = std::env::current_dir().unwrap().join("target/test_tmp");
    let input_dir = test_dir.join("input");
    let output_dir = test_dir.join("output");

    let _ = fs::remove_dir_all(&test_dir);
    fs::create_dir_all(&input_dir).unwrap();
    fs::create_dir_all(&output_dir).unwrap();

    // 2. 生成一张简单的测试线条图片 (白底黑线方块)
    let width = 100;
    let height = 100;
    let mut img = ImageBuffer::new(width, height);

    for (x, y, pixel) in img.enumerate_pixels_mut() {
        if (x == 20 || x == 80) && (y >= 20 && y <= 80)
            || (y == 20 || y == 80) && (x >= 20 && x <= 80)
        {
            *pixel = Rgb([0u8, 0u8, 0u8]); // 黑色线条
        } else {
            *pixel = Rgb([255u8, 255u8, 255u8]); // 白色背景
        }
    }

    let img_path = input_dir.join("square.bmp");
    img.save(&img_path).unwrap();
    assert!(img_path.exists());

    // 3. 配置 AppConfig
    let mut config = AppConfig::default();
    config.input_dir = input_dir.clone();
    config.output_dir = output_dir.clone();
    config.vectorize.threshold = 128;
    config.vectorize.rdp_epsilon = 0.5;

    let output_plt_path = output_dir.join("square.plt");

    // 4. 执行转换
    convert_image_to_plt(&img_path, &output_plt_path, &config).expect("图像转换应成功");

    // 5. 校验结果
    assert!(output_plt_path.exists(), "生成的 PLT 文件应存在");

    let plt_content = fs::read_to_string(&output_plt_path).unwrap();
    assert!(plt_content.starts_with("IN;"), "PLT 文件必须以 IN; 开始");
    assert!(plt_content.contains("SP1;"), "必须包含选笔指令 SP1;");
    assert!(plt_content.contains("PU"), "必须包含抬笔指令");
    assert!(plt_content.contains("PD"), "必须包含落笔指令");
    assert!(plt_content.ends_with("SP0;\n"), "必须以 SP0; 结尾");
    assert_closed_paths(&plt_content);
}

fn assert_closed_paths(plt: &str) {
    let mut start = None;
    let mut drawn_paths = 0;
    for command in plt.split(';').map(str::trim) {
        if let Some(coords) = command.strip_prefix("PU") {
            start = Some(coords.to_string());
        } else if let Some(coords) = command.strip_prefix("PD") {
            let values: Vec<_> = coords.split(',').collect();
            assert!(values.len() >= 4);
            let end = values[values.len() - 2..].join(",");
            assert_eq!(Some(&end), start.as_ref(), "轮廓必须落笔回到起点");
            drawn_paths += 1;
        }
    }
    assert!(drawn_paths > 0);
}

#[test]
fn test_bmp_foreground_and_closed_contours() {
    use image2plt::converter::{preprocessor, vectorize};
    use std::io::Cursor;

    for invert in [false, true] {
        let background: u8 = if invert { 0 } else { 255 };
        let foreground = 255 - background;
        let mut img = ImageBuffer::from_pixel(40, 40, Rgb([background; 3]));
        for y in 10..30 {
            for x in 10..30 {
                img.put_pixel(x, y, Rgb([foreground; 3]));
            }
        }
        let mut bytes = Cursor::new(Vec::new());
        img.write_to(&mut bytes, image::ImageFormat::Bmp).unwrap();
        let decoded = image::load_from_memory(bytes.get_ref()).unwrap();
        let mut config = AppConfig::default();
        config.vectorize.invert = invert;
        // 过滤原始噪点，不应将简化后的矩形误删。
        config.vectorize.min_points = 10;
        let binary = preprocessor::preprocess(&decoded, &config.vectorize);
        assert_eq!(binary.get_pixel(0, 0).0[0], 0);
        assert_eq!(binary.get_pixel(20, 20).0[0], 255);
        let paths = vectorize::extract_polylines(&binary, &config.vectorize);
        assert_eq!(paths.len(), 1, "只能提取矩形，不能提取背景外框");
        assert!(paths[0].len() >= 4);
        assert_eq!(paths[0].first(), paths[0].last());
    }
}

#[test]
fn test_l_shape_preserves_pixel_boundaries() {
    for scale in [1, 10] {
        for padding in [0, 3] {
            for invert in [false, true] {
                for threshold in [0, 128] {
                    let background = if invert { 0u8 } else { 255 };
                    let size = scale * 2 + padding * 2;
                    let img = ImageBuffer::from_fn(size, size, |x, y| {
                        let in_shape = x >= padding
                            && y >= padding
                            && x < padding + 2 * scale
                            && y < padding + 2 * scale
                            && (x < padding + scale || y < padding + scale);
                        Rgb([if in_shape {
                            255 - background
                        } else {
                            background
                        }; 3])
                    });
                    let mut config = AppConfig::default();
                    config.vectorize.invert = invert;
                    config.vectorize.threshold = threshold;
                    let binary = preprocessor::preprocess(&img.into(), &config.vectorize);
                    for (x, y, pixel) in binary.enumerate_pixels() {
                        let in_shape = x >= padding
                            && y >= padding
                            && x < padding + 2 * scale
                            && y < padding + 2 * scale
                            && (x < padding + scale || y < padding + scale);
                        assert_eq!(pixel.0[0] != 0, in_shape, "预处理不能改变黑白格 ({x}, {y})");
                    }
                    let expected: Vec<_> = [(0, 0), (2, 0), (2, 1), (1, 1), (1, 2), (0, 2), (0, 0)]
                        .into_iter()
                        .map(|(x, y)| {
                            Point::new((x * scale + padding) as f64, (y * scale + padding) as f64)
                        })
                        .collect();
                    for mode in ["contour", "vtracer"] {
                        config.vectorize.mode = mode.into();
                        let paths = vectorize::extract_polylines(&binary, &config.vectorize);
                        assert_eq!(paths.len(), 1, "L 形应只有一个闭合外轮廓");
                        assert_eq!(paths[0], expected, "L 形必须保留六个角，包括右下方的凹角");
                    }
                }
            }
        }
    }
}

#[test]
fn test_l_shape_bmp_to_plt() {
    let test_dir = std::env::current_dir()
        .unwrap()
        .join("target/l_shape_regression");
    fs::create_dir_all(&test_dir).unwrap();
    let input = test_dir.join("l_shape.bmp");
    let output = test_dir.join("l_shape.plt");
    // [[1, 1], [1, 0]]：1 是黑色，0 是白色。
    let img = ImageBuffer::from_fn(2, 2, |x, y| {
        Rgb([if x == 1 && y == 1 { 255u8 } else { 0 }; 3])
    });
    img.save(&input).unwrap();
    let mut config = AppConfig::default();
    config.plt.output_width_mm = 2.0;
    convert_image_to_plt(&input, &output, &config).unwrap();
    let plt = fs::read_to_string(output).unwrap();
    assert_eq!(
        plt,
        "IN;\nSP1;\nPU0,80;\nPM0;\nPD80,80,80,40,40,40,40,0,0,0,0,80;\nPM2;\nFT1;\nFP0;\nPU;\nSP0;\n"
    );
}
