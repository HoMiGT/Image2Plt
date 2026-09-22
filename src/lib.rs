//! `image2plt` 核心库，提供将图像转换为 HP-GL (PLT) 矢量图的核心引擎与配置功能。
//! 
//! 这个库主要被拆分为两个核心模块：
//! - [`config`]: 负责处理和定义各种矢量化与物理尺寸的配置参数。
//! - [`converter`]: 提供了所有核心转换逻辑，包含图像预处理、矢量化提取及最终的 PLT 编码流生成。

pub mod config;
pub mod converter;
