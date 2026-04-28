use std::process::Command;

const UNINSTALL_REG_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\realtime-translator";
const UNINSTALL_VALUE_NAME: &str = "UninstallString";

/// 从 Windows 注册表读取卸载命令并启动卸载器
#[tauri::command]
pub fn uninstall_app(app: tauri::AppHandle) -> Result<(), String> {
    let uninstall_str = read_uninstall_string()?;

    // 解析带引号的路径：去掉首尾引号，分离可执行文件和参数
    let (exe, args) = parse_uninstall_command(&uninstall_str);

    log::info!("启动卸载程序: {} {:?}", exe, args);

    // 启动卸载器
    Command::new(&exe)
        .args(&args)
        .spawn()
        .map_err(|e| format!("启动卸载程序失败: {}", e))?;

    // 退出应用，让卸载器可以删除安装目录
    app.exit(0);
    Ok(())
}

/// 通过 reg query 读取 UninstallString
fn read_uninstall_string() -> Result<String, String> {
    let output = Command::new("reg")
        .args([
            "query",
            &format!("HKCU\\{}", UNINSTALL_REG_KEY),
            "/v",
            UNINSTALL_VALUE_NAME,
        ])
        .output()
        .map_err(|e| format!("执行 reg query 失败: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("注册表查询失败: {}", stderr));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);

    // 解析输出格式：UninstallString    REG_SZ    "C:\path\uninstall.exe"
    for line in stdout.lines() {
        let line = line.trim();
        if line.starts_with(UNINSTALL_VALUE_NAME) {
            // 按类型标识定位值的起始位置，比按空格分割更可靠
            for type_name in &["REG_EXPAND_SZ", "REG_SZ"] {
                if let Some(idx) = line.find(type_name) {
                    let value = line[idx + type_name.len()..].trim();
                    if !value.is_empty() {
                        return Ok(value.to_string());
                    }
                }
            }
        }
    }

    Err("未在注册表输出中找到 UninstallString 值".to_string())
}

/// 解析卸载命令：处理带引号的路径 + 参数
fn parse_uninstall_command(cmd: &str) -> (String, Vec<String>) {
    let trimmed = cmd.trim();

    if trimmed.starts_with('"') {
        // 带引号的路径："C:\path\uninstall.exe" /S
        if let Some(end_quote) = trimmed[1..].find('"') {
            let exe = &trimmed[1..1 + end_quote];
            let rest = trimmed[2 + end_quote..].trim();
            let args: Vec<String> = if rest.is_empty() {
                Vec::new()
            } else {
                rest.split_whitespace().map(String::from).collect()
            };
            return (exe.to_string(), args);
        }
    }

    // 无引号：按空格分割，第一段为 exe
    let parts: Vec<&str> = trimmed.splitn(2, ' ').collect();
    let exe = parts[0].to_string();
    let args = if parts.len() > 1 {
        parts[1].split_whitespace().map(String::from).collect()
    } else {
        Vec::new()
    };

    (exe, args)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_quoted_path_with_args() {
        let (exe, args) = parse_uninstall_command(
            r#""C:\Users\mu\AppData\Local\realtime-translator\uninstall.exe" /S"#,
        );
        assert_eq!(
            exe,
            r"C:\Users\mu\AppData\Local\realtime-translator\uninstall.exe"
        );
        assert_eq!(args, vec!["/S"]);
    }

    #[test]
    fn parse_quoted_path_no_args() {
        let (exe, args) = parse_uninstall_command(r#""C:\path\uninstall.exe""#);
        assert_eq!(exe, r"C:\path\uninstall.exe");
        assert!(args.is_empty());
    }

    #[test]
    fn parse_unquoted_path() {
        let (exe, args) = parse_uninstall_command("C:\\path\\uninstall.exe");
        assert_eq!(exe, "C:\\path\\uninstall.exe");
        assert!(args.is_empty());
    }
}
