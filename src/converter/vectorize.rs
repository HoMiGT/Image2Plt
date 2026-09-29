//! 图像矢量化和多边形抽取模块。
//!
//! 沿二值化图像的前景像素外边界追踪闭合轮廓，
//! 并利用 Ramer-Douglas-Peucker (RDP) 算法来简化过密集的折线节点，输出最终的绘图路径。

use super::{Point, Polyline};
use crate::config::VectorizeConfig;
use image::GrayImage;
use std::collections::HashMap;

/// 提取图像中的所有矢量折线路径。
///
/// 这是一个入口策略函数，根据配置的模式选择具体的底层提取算法。
///
/// # Arguments
///
/// * `binary_img` - 经过二值化预处理的图像。
/// * `config` - 矢量化相关的参数配置（例如 `rdp_epsilon` 和 `min_points`）。
///
/// # Returns
///
/// 返回一个 `Vec<Polyline>`，其中每个 `Polyline` 是一条独立的、由多点组成的连续路径。
pub fn extract_polylines(binary_img: &GrayImage, config: &VectorizeConfig) -> Vec<Polyline> {
    if config.mode == "vtracer" {
        // vtracer 高级位图转矢量模式
        extract_via_vtracer(binary_img, config)
    } else {
        // 轮廓提取模式 (默认，高性能二值边界追踪)
        extract_via_contours(binary_img, config)
    }
}

/// 追踪像素方格的外边界，合并共线点并按配置做 RDP 简化。
///
/// # Arguments
///
/// * `binary_img` - 二值化图像。
/// * `config` - 配置对象。
///
/// # Returns
///
/// 返回化简后的折线数组。
fn extract_via_contours(binary_img: &GrayImage, config: &VectorizeConfig) -> Vec<Polyline> {
    let (width, height) = binary_img.dimensions();
    let mut edges = Vec::new();
    let mut outgoing = HashMap::new();
    let foreground = |x, y| binary_img.get_pixel(x, y).0[0] != 0;

    // 每个前景像素占据 [x, x+1] × [y, y+1]；只留下与背景相邻的边。
    // 方向依次为右、下、左、上，保证前景始终在有向边的右侧。
    let mut add_edge = |start, end, direction| {
        let index = edges.len();
        edges.push(BoundaryEdge {
            start,
            end,
            direction,
        });
        outgoing.entry(start).or_insert([None; 4])[direction] = Some(index);
    };
    for y in 0..height {
        for x in 0..width {
            if !foreground(x, y) {
                continue;
            }
            if y == 0 || !foreground(x, y - 1) {
                add_edge((x, y), (x + 1, y), 0);
            }
            if x + 1 == width || !foreground(x + 1, y) {
                add_edge((x + 1, y), (x + 1, y + 1), 1);
            }
            if y + 1 == height || !foreground(x, y + 1) {
                add_edge((x + 1, y + 1), (x, y + 1), 2);
            }
            if x == 0 || !foreground(x - 1, y) {
                add_edge((x, y + 1), (x, y), 3);
            }
        }
    }

    let mut visited = vec![false; edges.len()];
    let mut result = Vec::new();
    for start in 0..edges.len() {
        if visited[start] {
            continue;
        }
        let mut raw_polyline = Vec::new();
        let mut current = start;
        loop {
            let edge = &edges[current];
            visited[current] = true;
            raw_polyline.push(Point::new(edge.start.0 as f64, edge.start.1 as f64));

            // 在仅对角接触的交点优先右转，避免把两个黑块连成穿过白格的斜线。
            let candidates = &outgoing[&edge.end];
            current = [
                (edge.direction + 1) % 4,
                edge.direction,
                (edge.direction + 3) % 4,
            ]
            .into_iter()
            .find_map(|direction| candidates[direction])
            .expect("像素边界必须连续闭合");
            if current == start {
                break;
            }
        }
        if raw_polyline.len() < config.min_points {
            continue;
        }
        let corners = merge_collinear_points(&raw_polyline);
        let simplified = rdp_simplify(&corners, config.rdp_epsilon);
        // 大容差不能将闭合区域退化成一个点或一条往返线。
        result.push(
            if simplified.len() >= 4 && signed_area(&simplified) != 0.0 {
                simplified
            } else {
                corners
            },
        );
    }
    result
}

struct BoundaryEdge {
    start: (u32, u32),
    end: (u32, u32),
    direction: usize,
}

/// 输入为不重复起点的闭合环；仅移除共线点，不改变任何拐角，输出显式闭合。
fn merge_collinear_points(points: &[Point]) -> Polyline {
    let mut corners = Vec::new();
    for i in 0..points.len() {
        let previous = points[(i + points.len() - 1) % points.len()];
        let current = points[i];
        let next = points[(i + 1) % points.len()];
        let cross = (current.x - previous.x) * (next.y - current.y)
            - (current.y - previous.y) * (next.x - current.x);
        if cross != 0.0 {
            corners.push(current);
        }
    }
    if let Some(&first) = corners.first() {
        corners.push(first);
    }
    corners
}

fn signed_area(points: &[Point]) -> f64 {
    points
        .windows(2)
        .map(|pair| pair[0].x * pair[1].y - pair[1].x * pair[0].y)
        .sum::<f64>()
        / 2.0
}

/// 基于 vtracer 引擎转换矢量（当前回退到轮廓提取）。
fn extract_via_vtracer(binary_img: &GrayImage, config: &VectorizeConfig) -> Vec<Polyline> {
    // fallback 到 contour 提取或扩展 SVG 解析
    extract_via_contours(binary_img, config)
}

/// Ramer-Douglas-Peucker (RDP) 折线化简算法。
///
/// 递归地将曲线上距离特征连接线小于 `epsilon` 的中间点丢弃，从而在保留形状特征的同时大幅减少点数。
///
/// # Arguments
///
/// * `points` - 原始密集折线节点的数组。
/// * `epsilon` - 允许的最大垂直偏离误差（单位通常为像素）。值越大，化简越剧烈。
///
/// # Returns
///
/// 返回被精简后的新折线节点集合。
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

/// 计算点 `p` 到由 `line_start` 和 `line_end` 确定的直线的垂直距离。
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
    use crate::config::AppConfig;
    use image::Luma;
    use std::collections::HashSet;

    #[test]
    fn test_all_small_masks_preserve_exposed_pixel_edges() {
        let mut config = AppConfig::default().vectorize;
        config.min_points = 0;
        // 穷举 3×3 布局，覆盖凹角、孔洞、贴边、细线、孤立点和对角接触。
        for mask in 0u16..512 {
            let img = GrayImage::from_fn(3, 3, |x, y| {
                Luma([if mask & (1 << (y * 3 + x)) != 0 {
                    255
                } else {
                    0
                }])
            });
            let mut expected = HashSet::new();
            for (x, y, pixel) in img.enumerate_pixels() {
                if pixel.0[0] == 0 {
                    continue;
                }
                let (x, y) = (x as i32, y as i32);
                let square = [(x, y), (x + 1, y), (x + 1, y + 1), (x, y + 1), (x, y)];
                for pair in square.windows(2) {
                    // 两个黑格的公共边抵消，剩下的就是正确的有向边界。
                    if !expected.remove(&(pair[1], pair[0])) {
                        expected.insert((pair[0], pair[1]));
                    }
                }
            }

            let paths = extract_polylines(&img, &config);
            let mut actual = HashSet::new();
            for path in &paths {
                assert!(path.len() >= 5);
                assert_eq!(path.first(), path.last());
                for segment in path.windows(2) {
                    assert!(segment[0].x == segment[1].x || segment[0].y == segment[1].y);
                    let mut point = (segment[0].x as i32, segment[0].y as i32);
                    let end = (segment[1].x as i32, segment[1].y as i32);
                    let step = ((end.0 - point.0).signum(), (end.1 - point.1).signum());
                    while point != end {
                        let next = (point.0 + step.0, point.1 + step.1);
                        assert!(actual.insert((point, next)), "边界不能重复绘制: {mask:09b}");
                        point = next;
                    }
                }
            }
            assert_eq!(actual, expected, "像素边界不一致: {mask:09b}");
            assert_eq!(
                paths.iter().map(|path| signed_area(path)).sum::<f64>(),
                mask.count_ones() as f64
            );
        }
    }

    #[test]
    fn test_holes_and_diagonal_components_remain_separate() {
        let config = AppConfig::default().vectorize;
        let ring = GrayImage::from_fn(3, 3, |x, y| Luma([if x == 1 && y == 1 { 0 } else { 255 }]));
        let paths = extract_polylines(&ring, &config);
        assert_eq!(paths.len(), 2);
        assert_eq!(signed_area(&paths[0]), 9.0);
        assert_eq!(signed_area(&paths[1]), -1.0);

        for diagonal in [true, false] {
            let img = GrayImage::from_fn(2, 2, |x, y| {
                Luma([if (x == y) == diagonal { 255 } else { 0 }])
            });
            let paths = extract_polylines(&img, &config);
            assert_eq!(paths.len(), 2);
            assert!(paths.iter().all(|path| signed_area(path) == 1.0));
        }
    }

    #[test]
    fn test_filter_before_simplification_and_reject_degenerate_rdp() {
        let img = GrayImage::from_pixel(2, 2, Luma([255]));
        let mut config = AppConfig::default().vectorize;
        config.min_points = 8;
        config.rdp_epsilon = 100.0;
        let paths = extract_polylines(&img, &config);
        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0].len(), 5);
        assert_eq!(signed_area(&paths[0]), 4.0);
        config.min_points = 9;
        assert!(extract_polylines(&img, &config).is_empty());
    }

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
