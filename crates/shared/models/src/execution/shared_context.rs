use super::context::RequestContext;
use std::sync::{Arc, Mutex};

pub type SharedRequestContext = Arc<Mutex<RequestContext>>;

impl From<RequestContext> for SharedRequestContext {
    fn from(context: RequestContext) -> Self {
        Self::new(Mutex::new(context))
    }
}

impl From<Arc<Mutex<Self>>> for RequestContext {
    fn from(shared: Arc<Mutex<Self>>) -> Self {
        let guard = shared
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        guard.clone()
    }
}
