//! 转换引擎核心模块，整合了从像素位图到矢量路径的完整流水线。
//!
//! 该模块协调以下子模块完成工作：
//! - `preprocessor`: 负责图像的二值化、去噪等预处理。
//! - `vectorize`: 负责提取边界轮廓并简化多边形。
//! - `plt_encoder`: 负责将矢量折线编码为合规的 HP-GL 格式文件。

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

/// 转换单张图像到 PLT 文件。
///
/// 这是一个高层协调函数，它将读取图像，进行预处理，提取矢量轮廓，
/// 然后编码为 HP-GL 格式并将其保存到指定的输出路径中。
///
/// # Arguments
///
/// * `input_path` - 待转换的输入图像文件路径。
/// * `output_path` - 生成的 PLT (HP-GL) 文件的目标输出路径。
/// * `config` - 应用程序的配置参数 `AppConfig` 的引用，包含了转换所需的所有参数。
///
/// # Errors
///
/// 若发生以下情况，将返回 `anyhow::Result` 错误：
/// - 无法打开、读取或解码输入路径指定的图像文件。
/// - 创建输出目录失败。
/// - 将生成的 PLT 指令流写入输出文件失败。
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
