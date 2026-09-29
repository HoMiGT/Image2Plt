//! HP-GL (PLT) 编码器模块。
//!
//! 负责将几何上的矢量折线转换成绘图仪/切割机可以识别的 HP-GL 控制指令，
//! 例如 `PU` (Pen Up), `PD` (Pen Down)、`SP` (Select Pen) 和 `FP` (Fill Polygon)。

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

    // 3. 将闭合轮廓放入同一个多边形缓冲区，奇偶填充会保留孔洞和孔洞中的黑色岛。
    // 不能逐个轮廓分别填充，否则内部的白色孔洞也会被涂黑。
    let should_fill =
        |path: &Polyline| config.fill && path.len() >= 4 && path.first() == path.last();
    let mut polygon_started = false;
    for polyline in polylines.iter().filter(|path| should_fill(path)) {
        if polygon_started {
            writeln!(plt, "PM1;").unwrap();
        }
        let (start_x, start_y) = transform_coord(polyline[0].x, polyline[0].y);
        writeln!(plt, "PU{},{};", start_x, start_y).unwrap();
        if !polygon_started {
            writeln!(plt, "PM0;").unwrap();
            polygon_started = true;
        }
        write_pen_down(&mut plt, polyline, &transform_coord);
    }
    if polygon_started {
        // FT1：实心；FP0：奇偶规则。只填充，不再用 EP 描边，避免边缘被笔宽加粗。
        // 多边形模式中的 PD 仅记录边界，供 FP 填充使用，不会直接绘制轮廓。
        plt.push_str("PM2;\nFT1;\nFP0;\n");
    }

    // 4. 未闭合路径或关闭填充时，仍按普通抬笔/落笔轨迹输出。
    for polyline in polylines.iter().filter(|path| !should_fill(path)) {
        if polyline.is_empty() {
            continue;
        }

        // 起始点：PU x0,y0;
        let (start_x, start_y) = transform_coord(polyline[0].x, polyline[0].y);
        writeln!(plt, "PU{},{};", start_x, start_y).unwrap();

        write_pen_down(&mut plt, polyline, &transform_coord);
    }

    // 5. 结尾还原状态
    writeln!(plt, "PU;").unwrap();
    writeln!(plt, "SP0;").unwrap();

    plt
}

fn write_pen_down(
    plt: &mut String,
    polyline: &Polyline,
    transform_coord: &impl Fn(f64, f64) -> (i64, i64),
) {
    if polyline.len() < 2 {
        return;
    }
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
            fill: true,
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
