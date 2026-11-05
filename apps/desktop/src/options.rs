use std::ffi::OsString;
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LaunchOptions {
    pub diagnostics: bool,
    pub data_dir: Option<PathBuf>,
    pub worker: Option<PathBuf>,
    pub mode: String,
    pub help: bool,
    pub diagnostic_size: Option<[u32; 2]>,
    pub diagnostic_scale: Option<u8>,
}

impl Default for LaunchOptions {
    fn default() -> Self {
        Self {
            diagnostics: false,
            data_dir: None,
            worker: None,
            mode: "normal".to_owned(),
            help: false,
            diagnostic_size: None,
            diagnostic_scale: None,
        }
    }
}

impl LaunchOptions {
    pub const HELP: &str = "Image Studio\n\n用法：image-studio-desktop [选项]\n\n  --diagnostics                 显式启用开发诊断界面\n  --data-dir PATH               使用隔离应用数据根\n  --diagnostic-worker PATH      指定已构建诊断程序（需 --diagnostics）\n  --diagnostic-mode MODE        诊断故障模式，默认 normal（需 --diagnostics）\n  --diagnostic-size WIDTHxHEIGHT 开发窗口尺寸（需 --diagnostics）\n  --diagnostic-scale 1|2        开发渲染比例 px/pt（需 --diagnostics）\n  --help                        显示帮助\n\n诊断不执行图片生成。\n系统 WebView（Windows 上为 WebView2）不可用导致无法显示界面时，以退出码 3 结束，原因写入 <数据根>/logs/startup-failure.log。";

    pub fn parse_from(args: impl IntoIterator<Item = OsString>) -> Result<Self, String> {
        let mut options = Self::default();
        let mut args = args.into_iter();
        let mut diagnostic_option = false;
        while let Some(argument) = args.next() {
            let flag = argument.to_str().ok_or("选项名称必须为有效 UTF-8")?;
            match flag {
                "--diagnostics" => options.diagnostics = true,
                "--help" | "-h" => options.help = true,
                "--data-dir" => options.data_dir = Some(PathBuf::from(value(&mut args, flag)?)),
                "--diagnostic-worker" => {
                    options.worker = Some(PathBuf::from(value(&mut args, flag)?));
                    diagnostic_option = true;
                }
                "--diagnostic-mode" => {
                    let value = value(&mut args, flag)?;
                    options.mode = value
                        .into_string()
                        .map_err(|_| "诊断模式必须为有效 UTF-8")?;
                    if !super::DIAGNOSTIC_MODES.contains(&options.mode.as_str()) {
                        return Err(format!("未知诊断模式：{}", options.mode));
                    }
                    diagnostic_option = true;
                }
                "--diagnostic-size" => {
                    let text = value(&mut args, flag)?
                        .into_string()
                        .map_err(|_| "窗口尺寸必须为 WIDTHxHEIGHT")?;
                    let (width, height) =
                        text.split_once('x').ok_or("窗口尺寸必须为 WIDTHxHEIGHT")?;
                    let width = width.parse::<u32>().map_err(|_| "窗口宽度无效")?;
                    let height = height.parse::<u32>().map_err(|_| "窗口高度无效")?;
                    if !(960..=3840).contains(&width) || !(640..=2160).contains(&height) {
                        return Err("开发窗口范围为 960x640 至 3840x2160".to_owned());
                    }
                    options.diagnostic_size = Some([width, height]);
                    diagnostic_option = true;
                }
                "--diagnostic-scale" => {
                    let scale = value(&mut args, flag)?
                        .into_string()
                        .map_err(|_| "渲染比例必须为 1 或 2")?;
                    options.diagnostic_scale = Some(match scale.as_str() {
                        "1" => 1,
                        "2" => 2,
                        _ => return Err("渲染比例必须为 1 或 2 px/pt".to_owned()),
                    });
                    diagnostic_option = true;
                }
                _ => return Err(format!("未知选项：{flag}；使用 --help 查看帮助")),
            }
        }
        if diagnostic_option && !options.diagnostics && !options.help {
            return Err("诊断选项需要显式指定 --diagnostics".to_owned());
        }
        Ok(options)
    }
}

fn value(args: &mut impl Iterator<Item = OsString>, flag: &str) -> Result<OsString, String> {
    let value = args.next().ok_or_else(|| format!("{flag} 缺少参数"))?;
    if value.is_empty() {
        return Err(format!("{flag} 参数不能为空"));
    }
    if value.to_str().is_some_and(|value| value.starts_with("--")) {
        return Err(format!("{flag} 缺少参数"));
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<OsString> {
        values.iter().map(OsString::from).collect()
    }

    #[test]
    fn ordinary_startup_does_not_enable_diagnostics() {
        let options = LaunchOptions::parse_from(args(&[])).unwrap();
        assert!(!options.diagnostics);
        assert!(options.worker.is_none());
        assert_eq!(options.mode, "normal");
    }

    #[test]
    fn paths_are_preserved_without_shell_interpretation() {
        let options = LaunchOptions::parse_from(args(&[
            "--diagnostics",
            "--data-dir",
            "/tmp/中文 空格/资料",
            "--diagnostic-worker",
            "/tmp/worker $(touch forbidden);",
            "--diagnostic-mode",
            "flood",
        ]))
        .unwrap();
        assert_eq!(
            options.data_dir.unwrap(),
            PathBuf::from("/tmp/中文 空格/资料")
        );
        assert_eq!(
            options.worker.unwrap(),
            PathBuf::from("/tmp/worker $(touch forbidden);")
        );
        assert_eq!(options.mode, "flood");
    }

    #[test]
    fn diagnostic_arguments_require_explicit_development_mode() {
        assert!(LaunchOptions::parse_from(args(&["--diagnostic-mode", "normal"])).is_err());
        assert!(LaunchOptions::parse_from(args(&["--diagnostic-worker", "/tmp/worker"])).is_err());
    }

    #[test]
    fn viewport_controls_are_bounded_and_development_only() {
        let options = LaunchOptions::parse_from(args(&[
            "--diagnostics",
            "--diagnostic-size",
            "960x640",
            "--diagnostic-scale",
            "1",
        ]))
        .unwrap();
        assert_eq!(options.diagnostic_size, Some([960, 640]));
        assert_eq!(options.diagnostic_scale, Some(1));
        for values in [
            vec!["--diagnostic-scale", "1"],
            vec!["--diagnostics", "--diagnostic-size", "0x0"],
            vec!["--diagnostics", "--diagnostic-size", "999999999x800"],
            vec!["--diagnostics", "--diagnostic-scale", "NaN"],
        ] {
            assert!(LaunchOptions::parse_from(args(&values)).is_err());
        }
    }

    #[test]
    fn missing_unknown_or_empty_arguments_are_rejected() {
        for input in [
            args(&["--data-dir"]),
            args(&["--data-dir", "--diagnostics"]),
            args(&["--data-dir", ""]),
            args(&["--unknown"]),
            args(&["--diagnostics", "--diagnostic-mode", "generate"]),
        ] {
            assert!(LaunchOptions::parse_from(input).is_err());
        }
    }
}
