//! HP-GL (PLT) 编码器模块。
//!
//! 负责将几何上的矢量折线转换成绘图仪/切割机可以识别的 HP-GL 控制指令，
//! 例如 `PU` (Pen Up), `PD` (Pen Down) 和 `SP` (Select Pen)。

use super::Polyline;
use crate::config::PltConfig;
use std::fmt::Write;

/// 将折线数组编码为合规的 HP-GL (PLT) 格式文本字符串。
///
/// 该函数会将给定的矢量路径缩放至目标物理尺寸，并根据配置决定是否进行 Y 轴反转，
/// 最终输出完整的设备控制指令序列。
///
/// # Arguments
///
/// * `polylines` - 需要被编码的一组折线，每个折线由连续的点构成。
/// * `img_width` - 原始图像的像素宽度。
/// * `img_height` - 原始图像的像素高度。
/// * `config` - 包含画笔编号、物理单位映射比率等信息的 PLT 编码配置。
///
/// # Returns
///
/// 返回一个包含合法 HP-GL 指令流的 `String`，可以直接写入到 `.plt` 扩展名的文件中。
pub fn encode_plt(
    polylines: &[Polyline],
    img_width: u32,
    img_height: u32,
    config: &PltConfig,
) -> String {
    let mut plt = String::with_capacity(1024 * 16);

    // 1. 初始化 HP-GL 绘图仪指令
    writeln!(plt, "IN;").unwrap();
    writeln!(plt, "SP{};", config.pen_number).unwrap();

    if polylines.is_empty() {
        writeln!(plt, "PU;").unwrap();
        writeln!(plt, "SP0;").unwrap();
        return plt;
    }

    // 2. 计算缩放因子 scale (像素 -> PLT units)
    let scale = if config.output_width_mm > 0.0 {
        (config.output_width_mm * config.units_per_mm) / (img_width as f64)
    } else {
        1.0
    };

    let img_h = img_height as f64;

    // 坐标转换闭包
    let transform_coord = |x: f64, y: f64| -> (i64, i64) {
        let px = (x * scale).round() as i64;
        let py = if config.flip_y {
            ((img_h - y) * scale).round() as i64
        } else {
            (y * scale).round() as i64
        };
        (px, py)
    };

    // 3. 编码轨迹指令
    for polyline in polylines {
        if polyline.is_empty() {
            continue;
        }

        // 起始点：PU x0,y0;
        let (start_x, start_y) = transform_coord(polyline[0].x, polyline[0].y);
        writeln!(plt, "PU{},{};", start_x, start_y).unwrap();

        if polyline.len() > 1 {
            // 后续节点：PD x1,y1,x2,y2...;
            plt.push_str("PD");
            for (idx, pt) in polyline.iter().enumerate().skip(1) {
                let (cx, cy) = transform_coord(pt.x, pt.y);
                if idx > 1 {
                    plt.push(',');
                }
                write!(plt, "{},{}", cx, cy).unwrap();
            }
            plt.push_str(";\n");
        }
    }

    // 4. 结尾还原状态
    writeln!(plt, "PU;").unwrap();
    writeln!(plt, "SP0;").unwrap();

    plt
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::converter::Point;

    #[test]
    fn test_encode_plt_format() {
        let config = PltConfig {
            pen_number: 1,
            units_per_mm: 40.0,
            output_width_mm: 100.0,
            flip_y: true,
        };

        let polylines = vec![vec![Point::new(0.0, 0.0), Point::new(10.0, 20.0)]];
        let plt = encode_plt(&polylines, 100, 100, &config);

        assert!(plt.contains("IN;"));
        assert!(plt.contains("SP1;"));
        assert!(plt.contains("PU0,4000;")); // y 翻转: (100 - 0) * 40 = 4000
        assert!(plt.contains("PD400,3200;")); // x: 10*40=400, y: (100-20)*40=3200
        assert!(plt.contains("SP0;"));
    }
}

