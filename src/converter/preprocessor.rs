use crate::config::VectorizeConfig;
use image::{DynamicImage, GrayImage};
use imageproc::contrast::{threshold, ThresholdType};
use imageproc::filter::gaussian_blur_f32;

/// 图像预处理 Pipeline：灰度化 -> 高斯滤波 -> 二值化 -> (可选)反转
pub fn preprocess(img: &DynamicImage, config: &VectorizeConfig) -> GrayImage {
    // 1. 转为 8 位灰度图
    let mut gray = img.to_luma8();

    // 2. 高斯轻微模糊以消除轻微像素噪点
    gray = gaussian_blur_f32(&gray, 0.8);

    // 3. 计算二值化阈值
    let target_threshold = if config.threshold == 0 {
        calculate_otsu_threshold(&gray)
    } else {
        config.threshold
    };

    // 4. 应用二值化 (大于 threshold 设为 255，否则设为 0)
    let binary = threshold(&gray, target_threshold, ThresholdType::Binary);


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

/// Otsu 自适应大津二值化阈值算法
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
