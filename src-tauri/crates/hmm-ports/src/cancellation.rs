pub trait CancellationToken: Send + Sync {
    fn is_cancelled(&self) -> bool;
}

pub struct NeverCancelled;

impl CancellationToken for NeverCancelled {
    fn is_cancelled(&self) -> bool {
        false
    }
}
