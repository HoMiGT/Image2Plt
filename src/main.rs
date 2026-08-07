use anyhow::{Context, Result};
use image2plt::config::AppConfig;
use image2plt::converter;
use rayon::prelude::*;

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;
use walkdir::WalkDir;

fn main() -> Result<()> {
    println!("==================================================");
    println!("  High-Performance Image to PLT (HP-GL) Converter ");
    println!("==================================================");

    // 1. 加载或生成配置文件
    let config_path = Path::new("config.toml");
    let config = AppConfig::load_or_create(config_path)
        .context("初始化配置文件失败")?;

    println!("[配置] 输入路径: {:?}", config.input_dir);
    println!("[配置] 输出路径: {:?}", config.output_dir);
    println!("[配置] 转换模式: {}", config.vectorize.mode);
    println!("[配置] RDP 容差 : {}", config.vectorize.rdp_epsilon);

    // 2. 确保目录存在
    if !config.input_dir.exists() {
        fs::create_dir_all(&config.input_dir)?;
        println!(
            "\n提示: 输入目录 {:?} 不存在，已自动创建。请将需要转换的图片放入该目录。",
            config.input_dir
        );
    }
    if !config.output_dir.exists() {
        fs::create_dir_all(&config.output_dir)?;
    }

    // 3. 收集输入目录中的所有图片文件
    let image_extensions = ["png", "jpg", "jpeg", "bmp", "webp", "tiff", "gif"];
    let files: Vec<PathBuf> = WalkDir::new(&config.input_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .map(|e| e.into_path())
        .filter(|path| {
            path.extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| image_extensions.contains(&ext.to_lowercase().as_str()))
                .unwrap_or(false)
        })
        .collect();

    if files.is_empty() {
        println!(
            "\n在目录 {:?} 中未发现支持的图片文件。支持的格式: {:?}",
            config.input_dir, image_extensions
        );
        return Ok(());
    }

    println!("\n发现 {} 个待转换图片文件，启动 Rayon 并行处理引擎...", files.len());
    let start_time = Instant::now();

    // 4. 使用 Rayon 并行处理图片转换
    let results: Vec<Result<PathBuf, String>> = files
        .par_iter()
        .map(|input_path| {
            let relative = input_path
                .strip_prefix(&config.input_dir)
                .unwrap_or(input_path);
            let mut output_path = config.output_dir.join(relative);
            output_path.set_extension("plt");

            match converter::convert_image_to_plt(input_path, &output_path, &config) {
                Ok(_) => Ok(output_path),
                Err(err) => Err(format!("处理 {:?} 失败: {:#}", input_path, err)),
            }
        })
        .collect();

    let total_time = start_time.elapsed();

    // 5. 统计与汇总
    let mut success_count = 0;
    let mut fail_count = 0;

    for res in results {
        match res {
            Ok(out_path) => {
                success_count += 1;
                println!("  [✓] 转换成功 -> {:?}", out_path);
            }
            Err(err_msg) => {
                fail_count += 1;
                eprintln!("  [✗] {}", err_msg);
            }
        }
    }

    println!("\n==================================================");
    println!(
        "处理完成! 成功: {}, 失败: {}, 总耗时: {:.2?}",
        success_count, fail_count, total_time
    );
    if success_count > 0 {
        let avg = total_time / success_count as u32;
        println!("平均每张耗时: {:.2?}", avg);
    }
    println!("==================================================");

    Ok(())
}
