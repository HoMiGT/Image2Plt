use image::{ImageBuffer, Rgb};
use image2plt::config::AppConfig;
use image2plt::converter::convert_image_to_plt;
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
        if (x == 20 || x == 80) && (y >= 20 && y <= 80) || (y == 20 || y == 80) && (x >= 20 && x <= 80) {
            *pixel = Rgb([0u8, 0u8, 0u8]); // 黑色线条
        } else {
            *pixel = Rgb([255u8, 255u8, 255u8]); // 白色背景
        }
    }

    let img_path = input_dir.join("square.png");
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
}
