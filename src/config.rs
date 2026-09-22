//! 提供应用程序和转换引擎所需要的所有配置定义及加载机制。
//! 
//! 支持从 `config.toml` 文件反序列化配置，也包含默认配置的生成。

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// 输入图片目录路径
    pub input_dir: PathBuf,
    /// 输出 PLT 矢量图目录路径
    pub output_dir: PathBuf,
    /// 矢量化参数配置
    pub vectorize: VectorizeConfig,
    /// PLT (HP-GL) 规范与物理尺寸配置
    pub plt: PltConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorizeConfig {
    /// 矢量化模式: "contour" (轮廓线/边缘检测模式，契合裁床/刻字机) 或 "vtracer" (位图形状填充模式)
    #[serde(default = "default_mode")]
    pub mode: String,

    /// 二值化阈值 (0 ~ 255)，设为 0 时自动触发 Otsu 自适应阈值算法
    #[serde(default = "default_threshold")]
    pub threshold: u8,

    /// 是否反转图像颜色 (白底黑线 vs 黑底白线)
    #[serde(default)]
    pub invert: bool,

    /// Ramer-Douglas-Peucker (RDP) 折线简化容差 (像素单位)，值越大线段越精简平滑
    #[serde(default = "default_rdp_epsilon")]
    pub rdp_epsilon: f64,

    /// 最短闭合/路径最小节点过滤，丢弃噪音噪点 (像素数)
    #[serde(default = "default_min_points")]
    pub min_points: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PltConfig {
    /// HP-GL 选笔指令 Pen Number (如 SP1;)
    #[serde(default = "default_pen_number")]
    pub pen_number: usize,

    /// 工业标准 HP-GL 单位映射: 1 mm 对应的 PLT units (标准 1 unit = 0.025 mm, 即 40 units/mm)
    #[serde(default = "default_units_per_mm")]
    pub units_per_mm: f64,

    /// 输出的目标物理宽度 (毫米 mm)，若为 0.0 则按图像原始像素与 units_per_mm 1:1 映射
    #[serde(default)]
    pub output_width_mm: f64,

    /// 是否翻转 Y 轴（计算机图像坐标系 Top-Left -> 绘图仪坐标系 Bottom-Left）
    #[serde(default = "default_true")]
    pub flip_y: bool,
}

/// 默认矢量化模式
fn default_mode() -> String {
    "contour".to_string()
}
/// 默认二值化阈值 (0 表示自适应)
fn default_threshold() -> u8 {
    0
}
/// 默认 RDP 容差
fn default_rdp_epsilon() -> f64 {
    1.0
}
/// 默认最小路径节点数
fn default_min_points() -> usize {
    3
}
/// 默认画笔编号
fn default_pen_number() -> usize {
    1
}
/// 默认单位转换率 (40.0 对应 0.025mm/unit)
fn default_units_per_mm() -> f64 {
    40.0
}
/// 默认是否翻转 Y 轴
fn default_true() -> bool {
    true
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            input_dir: PathBuf::from("./input"),
            output_dir: PathBuf::from("./output"),
            vectorize: VectorizeConfig {
                mode: "contour".to_string(),
                threshold: 0,
                invert: false,
                rdp_epsilon: 1.0,
                min_points: 3,
            },
            plt: PltConfig {
                pen_number: 1,
                units_per_mm: 40.0,
                output_width_mm: 200.0,
                flip_y: true,
            },
        }
    }
}

impl AppConfig {
    /// 从指定 toml 文件加载配置，若文件不存在则创建默认配置。
    ///
    /// # Arguments
    ///
    /// * `path` - 配置文件的路径。
    ///
    /// # Errors
    ///
    /// 若发生以下情况，将返回 `anyhow::Result` 错误：
    /// - 配置文件存在，但读取其内容失败。
    /// - 配置文件的内容不是有效的 TOML 格式。
    /// - 配置文件不存在，且在尝试创建默认文件时写入失败。
    pub fn load_or_create<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        if path.exists() {
            let content = fs::read_to_string(path)
                .with_context(|| format!("读取配置文件 {:?} 失败", path))?;
            let config: AppConfig = toml::from_str(&content)
                .with_context(|| format!("解析配置文件 {:?} 格式错误", path))?;
            Ok(config)
        } else {
            let config = AppConfig::default();
            let toml_str = toml::to_string_pretty(&config)?;
            fs::write(path, toml_str)?;
            println!("已自动创建默认配置文件: {:?}", path);
            Ok(config)
        }
    }
}
