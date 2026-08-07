use super::{Point, Polyline};
use crate::config::VectorizeConfig;
use image::GrayImage;
use imageproc::contours::find_contours;

/// 提取图像中的所有矢量折线路径
pub fn extract_polylines(binary_img: &GrayImage, config: &VectorizeConfig) -> Vec<Polyline> {
    if config.mode == "vtracer" {
        // vtracer 高级位图转矢量模式
        extract_via_vtracer(binary_img, config)
    } else {
        // 轮廓提取模式 (默认，高性能二值边界追踪)
        extract_via_contours(binary_img, config)
    }
}

/// 采用 Suzuki 算法寻找边界轮廓并做 RDP 简化
fn extract_via_contours(binary_img: &GrayImage, config: &VectorizeConfig) -> Vec<Polyline> {
    // 找到所有前景色 (非零) 的闭合轮廓
    let contours = find_contours::<u32>(binary_img);


    let mut result = Vec::new();

    for contour in contours {
        if contour.points.len() < config.min_points {
            continue;
        }

        // 转为 Point 序列
        let raw_polyline: Polyline = contour
            .points
            .iter()
            .map(|p| Point::new(p.x as f64, p.y as f64))
            .collect();

        // 应用 Ramer-Douglas-Peucker (RDP) 简化折线
        let simplified = rdp_simplify(&raw_polyline, config.rdp_epsilon);

        if simplified.len() >= config.min_points {
            result.push(simplified);
        }
    }

    result
}

/// 基于 vtracer 引擎转换矢量
fn extract_via_vtracer(binary_img: &GrayImage, config: &VectorizeConfig) -> Vec<Polyline> {
    // fallback 到 contour 提取或扩展 SVG 解析
    extract_via_contours(binary_img, config)
}

/// Ramer-Douglas-Peucker (RDP) 折线化简算法
pub fn rdp_simplify(points: &[Point], epsilon: f64) -> Vec<Point> {
    if points.len() <= 2 || epsilon <= 0.0 {
        return points.to_vec();
    }

    let mut max_dist = 0.0;
    let mut index = 0;

    let start = points[0];
    let end = points[points.len() - 1];

    for i in 1..(points.len() - 1) {
        let dist = perpendicular_distance(points[i], start, end);
        if dist > max_dist {
            max_dist = dist;
            index = i;
        }
    }

    if max_dist > epsilon {
        // 递归拆分
        let left = rdp_simplify(&points[..=index], epsilon);
        let right = rdp_simplify(&points[index..], epsilon);

        // 合并结果（去除重复的分割点）
        let mut result = left;
        result.pop();
        result.extend(right);
        result
    } else {
        vec![start, end]
    }
}

/// 计算点 p 到由 line_start 和 line_end 确定的直线的垂直距离
fn perpendicular_distance(p: Point, line_start: Point, line_end: Point) -> f64 {
    let dx = line_end.x - line_start.x;
    let dy = line_end.y - line_start.y;

    let norm = (dx * dx + dy * dy).sqrt();
    if norm == 0.0 {
        // line_start 和 line_end 重合
        let pdx = p.x - line_start.x;
        let pdy = p.y - line_start.y;
        return (pdx * pdx + pdy * pdy).sqrt();
    }

    let num = ((line_end.y - line_start.y) * p.x - (line_end.x - line_start.x) * p.y
        + line_end.x * line_start.y
        - line_end.y * line_start.x)
        .abs();

    num / norm
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rdp_simplify() {
        // 直线上包含中间共线点
        let line = vec![
            Point::new(0.0, 0.0),
            Point::new(5.0, 0.1), // 几乎共线
            Point::new(10.0, 0.0),
        ];

        let simplified = rdp_simplify(&line, 0.5);
        assert_eq!(simplified.len(), 2);
        assert_eq!(simplified[0], Point::new(0.0, 0.0));
        assert_eq!(simplified[1], Point::new(10.0, 0.0));
    }
}

