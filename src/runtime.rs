use std::sync::OnceLock;
use tokio::runtime::{Handle, Runtime};

static RUNTIME: OnceLock<Runtime> = OnceLock::new();

pub fn init() -> &'static Runtime {
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .thread_name("vibebar-worker")
            .build()
            .expect("failed to build vibebar runtime")
    })
}

pub fn handle() -> Handle {
    init().handle().clone()
}
