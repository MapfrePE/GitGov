mod queue;

pub use queue::*;

pub fn init_outbox(app_data_dir: &std::path::Path) -> Result<Outbox, OutboxError> {
    Outbox::new(app_data_dir)
}
