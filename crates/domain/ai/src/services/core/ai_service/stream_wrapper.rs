use futures::Stream;
use std::pin::Pin;
use std::task::{Context, Poll};
use uuid::Uuid;

use crate::models::ai::{AiRequest, AiResponse};
use crate::models::RequestStatus;
use crate::services::core::request_storage::{RequestStorage, StoreParams};

pub struct StreamStorageWrapper {
    inner: Pin<Box<dyn Stream<Item = anyhow::Result<String>> + Send>>,
    storage: RequestStorage,
    request: AiRequest,
    request_id: Uuid,
    start: std::time::Instant,
    provider: String,
    model: String,
    accumulated: String,
    completed: bool,
}

impl StreamStorageWrapper {
    pub fn new(
        inner: Pin<Box<dyn Stream<Item = anyhow::Result<String>> + Send>>,
        storage: RequestStorage,
        request: AiRequest,
        request_id: Uuid,
        start: std::time::Instant,
        provider: String,
        model: String,
    ) -> Self {
        Self {
            inner,
            storage,
            request,
            request_id,
            start,
            provider,
            model,
            accumulated: String::new(),
            completed: false,
        }
    }

    fn store_completion(&self) {
        let response = AiResponse::new(
            self.request_id,
            self.accumulated.clone(),
            self.provider.clone(),
            self.model.clone(),
        )
        .with_latency(self.start.elapsed().as_millis() as u64)
        .with_streaming(true);

        self.storage.store(&StoreParams {
            request: &self.request,
            response: &response,
            context: &self.request.context,
            status: RequestStatus::Completed,
            error_message: None,
            cost_microdollars: 0,
        });
    }

    fn store_error(&self, error: &anyhow::Error) {
        let response = AiResponse::new(
            self.request_id,
            self.accumulated.clone(),
            self.provider.clone(),
            self.model.clone(),
        )
        .with_latency(self.start.elapsed().as_millis() as u64)
        .with_streaming(true);

        self.storage.store(&StoreParams {
            request: &self.request,
            response: &response,
            context: &self.request.context,
            status: RequestStatus::Failed,
            error_message: Some(&error.to_string()),
            cost_microdollars: 0,
        });
    }
}

impl Stream for StreamStorageWrapper {
    type Item = anyhow::Result<String>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match self.inner.as_mut().poll_next(cx) {
            Poll::Ready(Some(Ok(text))) => {
                self.accumulated.push_str(&text);
                Poll::Ready(Some(Ok(text)))
            },
            Poll::Ready(Some(Err(e))) => {
                if !self.completed {
                    self.completed = true;
                    self.store_error(&e);
                }
                Poll::Ready(Some(Err(e)))
            },
            Poll::Ready(None) => {
                if !self.completed {
                    self.completed = true;
                    self.store_completion();
                }
                Poll::Ready(None)
            },
            Poll::Pending => Poll::Pending,
        }
    }
}
