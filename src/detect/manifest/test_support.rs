use std::path::PathBuf;

thread_local! {
    static CONFIG_DIR: std::cell::RefCell<Option<PathBuf>> = const { std::cell::RefCell::new(None) };
}

pub(super) struct ConfigDirGuard(Option<PathBuf>);

impl Drop for ConfigDirGuard {
    fn drop(&mut self) {
        CONFIG_DIR.with(|dir| *dir.borrow_mut() = self.0.take());
    }
}

pub(super) fn set_config_dir(path: PathBuf) -> ConfigDirGuard {
    CONFIG_DIR.with(|dir| ConfigDirGuard(dir.replace(Some(path))))
}

pub(super) fn manifest_config_dir() -> Option<PathBuf> {
    CONFIG_DIR.with(|dir| dir.borrow().clone())
}
