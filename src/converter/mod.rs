pub mod plt_encoder;
pub mod preprocessor;
pub mod vectorize;

use crate::config::AppConfig;
use anyhow::{Context, Result};
use std::fs;
use std::path::Path;


/// 二维二维点 (物理/像素)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

/// 一条连续画笔路径 (多边形/折线)
pub type Polyline = Vec<Point>;

/// 转换单张图像到 PLT 文件
pub fn convert_image_to_plt<P1: AsRef<Path>, P2: AsRef<Path>>(
    input_path: P1,
    output_path: P2,
    config: &AppConfig,
) -> Result<()> {
    let input_path = input_path.as_ref();
    let output_path = output_path.as_ref();

    // 1. 加载图像
    let img = image::open(input_path)
        .with_context(|| format!("无法打开或解码图像文件: {:?}", input_path))?;

    let (width, height) = (img.width(), img.height());

    // 2. 预处理与二值化
    let binary_img = preprocessor::preprocess(&img, &config.vectorize);

    // 3. 矢量化 & 提取折线路径
    let polylines = vectorize::extract_polylines(&binary_img, &config.vectorize);

    // 4. 生成 PLT (HP-GL) 指令流
    let plt_content = plt_encoder::encode_plt(&polylines, width, height, &config.plt);

    // 5. 确保目标输出目录存在并保存
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(output_path, plt_content)
        .with_context(|| format!("无法写入 PLT 文件: {:?}", output_path))?;

    Ok(())
}
