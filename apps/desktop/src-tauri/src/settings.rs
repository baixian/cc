use crate::models::{ErrorDto, SettingsDto};
use std::{fs, io::Write, path::PathBuf, sync::Mutex};

pub struct SettingsStore {
    path: PathBuf,
    lock: Mutex<()>,
}

impl SettingsStore {
    pub fn new(config_dir: PathBuf) -> Self {
        Self { path: config_dir.join("preferences.json"), lock: Mutex::new(()) }
    }

    pub fn get(&self) -> Result<SettingsDto, ErrorDto> {
        let _guard = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        self.read()
    }

    pub fn update(&self, expected_revision: u32, refresh_interval_seconds: u32) -> Result<SettingsDto, ErrorDto> {
        if !(2..=30).contains(&refresh_interval_seconds) {
            return Err(ErrorDto::new("INVALID_SETTINGS", "刷新间隔仅允许 2 至 30 秒", false));
        }
        let _guard = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        let mut current = self.read()?;
        if current.revision != expected_revision {
            return Err(ErrorDto::new("REVISION_CONFLICT", "设置已在其他窗口改变，请刷新后重试", true));
        }
        current.revision = current.revision.checked_add(1)
            .ok_or_else(|| ErrorDto::new("REVISION_EXHAUSTED", "设置版本已达到上限", false))?;
        current.refresh_interval_seconds = refresh_interval_seconds;
        let dir = self.path.parent().ok_or_else(|| ErrorDto::new("SETTINGS_IO_ERROR", "设置目录不可用", false))?;
        fs::create_dir_all(dir).map_err(|_| ErrorDto::new("SETTINGS_IO_ERROR", "无法创建设置目录", true))?;
        let serialized = serde_json::to_vec(&current)
            .map_err(|_| ErrorDto::new("SETTINGS_INVALID", "无法序列化设置", false))?;
        let temporary = self.path.with_file_name(format!("preferences-{}.tmp", uuid::Uuid::new_v4()));
        let result = (|| -> std::io::Result<()> {
            let mut file = fs::OpenOptions::new().write(true).create_new(true).open(&temporary)?;
            file.write_all(&serialized)?;
            file.sync_all()?;
            fs::rename(&temporary, &self.path)?;
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
            return Err(ErrorDto::new("SETTINGS_IO_ERROR", "无法保存设置", true));
        }
        Ok(current)
    }

    fn read(&self) -> Result<SettingsDto, ErrorDto> {
        let bytes = match fs::read(&self.path) {
            Ok(bytes) if bytes.len() <= 4096 => bytes,
            Ok(_) => return Err(ErrorDto::new("SETTINGS_INVALID", "设置文件超过大小上限", false)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(SettingsDto::default()),
            Err(_) => return Err(ErrorDto::new("SETTINGS_IO_ERROR", "无法读取设置", true)),
        };
        let settings: SettingsDto = serde_json::from_slice(&bytes)
            .map_err(|_| ErrorDto::new("SETTINGS_INVALID", "设置文件格式无效，需要人工检查", false))?;
        if settings.schema_version != 1 || !(2..=30).contains(&settings.refresh_interval_seconds) {
            return Err(ErrorDto::new("SETTINGS_INVALID", "设置文件版本或字段无效，需要人工检查", false));
        }
        Ok(settings)
    }
}
