use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
};

use tauri::{path::BaseDirectory, AppHandle, Manager};

const USER_SCRIPTS_DIRECTORY: &str = "scripts";
const BUNDLED_TEMPLATE_DIRECTORY: &str = "examples";
const BUNDLED_TEMPLATES: &[&str] = &[
    "timestamp.py",
    "qr.py",
    "open_path.py",
    "script_template.py",
    "README.md",
];

pub fn provision_bundled_templates(app: &AppHandle) -> Result<(), String> {
    let source = app
        .path()
        .resolve(BUNDLED_TEMPLATE_DIRECTORY, BaseDirectory::Resource)
        .map_err(|error| format!("无法定位内置脚本模板：{error}"))?;
    let destination = app
        .path()
        .app_config_dir()
        .map_err(|error| format!("无法定位用户脚本目录：{error}"))?
        .join(USER_SCRIPTS_DIRECTORY);
    provision_from_directory(&source, &destination)
}

fn provision_from_directory(source: &Path, destination: &Path) -> Result<(), String> {
    fs::create_dir_all(destination)
        .map_err(|error| format!("无法创建用户脚本目录 {}：{error}", destination.display()))?;
    for name in BUNDLED_TEMPLATES {
        copy_template_if_missing(&source.join(name), &destination.join(name))?;
    }
    Ok(())
}

fn copy_template_if_missing(source: &Path, destination: &Path) -> Result<bool, String> {
    if destination.exists() {
        return Ok(false);
    }
    let bytes = fs::read(source)
        .map_err(|error| format!("无法读取内置脚本模板 {}：{error}", source.display()))?;
    let mut destination_file = match OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)
    {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => return Ok(false),
        Err(error) => {
            return Err(format!(
                "无法初始化用户脚本 {}：{error}",
                destination.display()
            ))
        }
    };
    if let Err(error) = destination_file
        .write_all(&bytes)
        .and_then(|_| destination_file.sync_all())
    {
        drop(destination_file);
        let _ = fs::remove_file(destination);
        return Err(format!(
            "无法写入用户脚本 {}：{error}",
            destination.display()
        ));
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{provision_from_directory, BUNDLED_TEMPLATES};

    fn temporary_root() -> std::path::PathBuf {
        std::env::temp_dir().join(format!("suo-user-scripts-test-{}", uuid::Uuid::new_v4()))
    }

    #[test]
    fn provisions_templates_without_overwriting_user_changes() {
        let root = temporary_root();
        let source = root.join("bundle");
        let destination = root.join("user-scripts");
        fs::create_dir_all(&source).expect("create source directory");
        for name in BUNDLED_TEMPLATES {
            fs::write(source.join(name), format!("bundled:{name}"))
                .expect("write bundled template");
        }

        provision_from_directory(&source, &destination).expect("provision templates");
        let timestamp = destination.join("timestamp.py");
        assert_eq!(
            fs::read_to_string(&timestamp).unwrap(),
            "bundled:timestamp.py"
        );

        fs::write(&timestamp, "user-edited").expect("edit user script");
        fs::write(source.join("timestamp.py"), "new-bundled-version")
            .expect("update bundled template");
        provision_from_directory(&source, &destination).expect("repeat provisioning");

        assert_eq!(fs::read_to_string(&timestamp).unwrap(), "user-edited");
        assert!(destination.join("open_path.py").is_file());
        assert!(destination.join("qr.py").is_file());
        assert!(destination.join("script_template.py").is_file());
        assert!(destination.join("README.md").is_file());
        let _ = fs::remove_dir_all(root);
    }
}
