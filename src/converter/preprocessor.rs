//! 图像预处理模块。
//!
//! 提供将彩色或灰度原始图像转换为去噪后的纯二值图像的功能，
//! 供后续矢量化步骤使用。

use crate::config::VectorizeConfig;
use image::{DynamicImage, GrayImage};
use imageproc::contrast::{ThresholdType, threshold};
use imageproc::filter::gaussian_blur_f32;

/// 图像预处理 Pipeline：灰度化 -> 高斯滤波 -> 二值化 -> (可选)反转。
///
/// # Arguments
///
/// * `img` - 原始输入的图像，可以是任意彩色或灰度格式。
/// * `config` - 矢量化配置，包含控制阈值、反转开关等参数。
///
/// # Returns
///
/// 返回处理完毕、准备提取轮廓的单通道二值化 `GrayImage` (像素值仅包含 0 或 255)。
pub fn preprocess(img: &DynamicImage, config: &VectorizeConfig) -> GrayImage {
    // 1. 转为 8 位灰度图
    let mut gray = img.to_luma8();

    // 2. 已经是纯黑白的图像直接保留像素，避免模糊抹掉细线、缺口或小色块。
    // 彩色/灰度图仍做轻微高斯去噪。
    if gray.pixels().any(|pixel| !matches!(pixel.0[0], 0 | 255)) {
        gray = gaussian_blur_f32(&gray, 0.8);
    }

    // 3. 计算二值化阈值
    let target_threshold = if config.threshold == 0 {
        calculate_otsu_threshold(&gray)
    } else {
        config.threshold
    };

    // 4. 轮廓算法以非零像素为前景：默认提取白底上的深色图形。
    let binary = threshold(&gray, target_threshold, ThresholdType::BinaryInverted);

    // 5. 根据配置判断是否反转颜色
    if config.invert {
        let mut inverted = binary.clone();
        for pixel in inverted.pixels_mut() {
            pixel.0[0] = 255 - pixel.0[0];
        }
        inverted
    } else {
        binary
    }
}

/// Otsu (大津法) 自适应二值化阈值算法。
///
/// 通过最大化类间方差，自动计算出图像的最佳全局二值化阈值。
/// 适用于背景和前景灰度分布呈现双峰特征的图像。
///
/// # Arguments
///
/// * `gray` - 单通道灰度图像。
///
/// # Returns
///
/// 返回计算得到的最佳阈值（0~255）。
fn calculate_otsu_threshold(gray: &GrayImage) -> u8 {
    let mut histogram = [0u64; 256];
    for pixel in gray.pixels() {
        histogram[pixel.0[0] as usize] += 1;
    }

    let total_pixels = (gray.width() * gray.height()) as f64;
    let mut sum = 0.0f64;
    for i in 0..256 {
        sum += (i as f64) * (histogram[i] as f64);
    }

    let mut sum_b = 0.0f64;
    let mut w_b = 0.0f64;
    let mut var_max = 0.0f64;
    let mut threshold_val = 128u8;

    for i in 0..256 {
        w_b += histogram[i] as f64;
        if w_b == 0.0 {
            continue;
        }
        let w_f = total_pixels - w_b;
        if w_f == 0.0 {
            break;
        }

        sum_b += (i as f64) * (histogram[i] as f64);
        let m_b = sum_b / w_b;
        let m_f = (sum - sum_b) / w_f;

        let var_between = w_b * w_f * (m_b - m_f) * (m_b - m_f);
        if var_between > var_max {
            var_max = var_between;
            threshold_val = i as u8;
        }
    }

    threshold_val
}
