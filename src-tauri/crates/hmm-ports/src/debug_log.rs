pub trait DebugLogControl: Send + Sync {
    fn is_enabled(&self) -> bool;
    fn set_enabled(&self, enabled: bool);
}

#[derive(Default)]
pub struct NoopDebugLogControl;

impl DebugLogControl for NoopDebugLogControl {
    fn is_enabled(&self) -> bool {
        false
    }

    fn set_enabled(&self, _enabled: bool) {}
}
