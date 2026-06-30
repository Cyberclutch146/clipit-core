use std::path::PathBuf;
use clipit_types::{ClipboardEvent, ClipboardContent};

// Dummy Error type for now
pub type Result<T> = std::result::Result<T, String>;

pub trait ClipboardProvider {
    fn start_monitoring<F>(&self, callback: F) -> Result<()> 
    where F: Fn(ClipboardEvent) + Send + Sync + 'static;
    fn stop_monitoring(&self) -> Result<()>;
    fn get_current_content(&self) -> Result<ClipboardContent>;
    fn set_content(&self, content: ClipboardContent) -> Result<()>;
}

pub struct Shortcut {
    pub modifiers: Vec<String>,
    pub key: String,
}

pub type HotkeyId = u32;

pub trait HotkeyProvider {
    fn register_hotkey<F>(&self, shortcut: Shortcut, callback: F) -> Result<HotkeyId>
    where F: Fn() + Send + Sync + 'static;
    fn unregister_hotkey(&self, id: HotkeyId) -> Result<()>;
}

pub struct Icon(pub Vec<u8>);
pub struct TrayMenu {} // Placeholder

pub trait TrayProvider {
    fn create_tray(&self, icon: Icon, menu: TrayMenu) -> Result<()>;
    fn update_tray(&self, icon: Option<Icon>, tooltip: Option<String>) -> Result<()>;
}
