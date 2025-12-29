use crate::messages::Message;
use crate::services::cloud_api;
use systemprompt_identifiers::ContextId;

use super::super::TuiApp;

impl TuiApp {
    pub(crate) fn spawn_refresh_conversations(&self) {
        let api_url = self.api_external_url.clone();
        let token = self.admin_token.clone();
        let sender = self.message_tx.clone();
        tokio::spawn(async move {
            match cloud_api::list_contexts(&api_url, &token).await {
                Ok(contexts) => {
                    let conversations: Vec<crate::state::ConversationDisplay> = contexts
                        .into_iter()
                        .map(|c| crate::state::ConversationDisplay {
                            context_id: c.context_id,
                            name: c.name,
                            task_count: c.task_count,
                            message_count: c.message_count,
                            last_message_at: c.last_message_at,
                            updated_at: Some(c.updated_at),
                        })
                        .collect();
                    if sender
                        .send(Message::ConversationsUpdate(conversations))
                        .is_err()
                    {
                        tracing::debug!("Message receiver dropped - UI may be shutting down");
                    }
                },
                Err(e) => {
                    tracing::error!("Failed to list contexts: {}", e);
                },
            }
        });
    }

    pub(crate) async fn select_conversation(&mut self, context_id: ContextId) {
        *self.current_context_id.write().await = context_id.clone();
        self.state.chat.set_context(context_id.clone());
        tracing::info!("Selected conversation: {}", context_id.as_ref());
        self.load_tasks_for_context(&context_id).await;
    }

    async fn load_tasks_for_context(&mut self, context_id: &ContextId) {
        let result = cloud_api::fetch_tasks_by_context(
            &self.api_external_url,
            &self.admin_token,
            context_id.as_str(),
        )
        .await;

        match result {
            Ok(tasks) => self.apply_loaded_tasks(tasks),
            Err(e) => tracing::error!("Failed to load tasks for conversation: {}", e),
        }
    }

    fn apply_loaded_tasks(&mut self, tasks: Vec<systemprompt_models::a2a::Task>) {
        tracing::info!("Loaded {} tasks for selected conversation", tasks.len());
        for task in tasks {
            self.state.chat.upsert_task(task);
        }
    }

    pub(crate) fn spawn_rename_conversation(&self, context_id: ContextId, name: String) {
        let api_url = self.api_external_url.clone();
        let token = self.admin_token.clone();
        let sender = self.message_tx.clone();
        tokio::spawn(async move {
            match cloud_api::update_context_name(&api_url, &token, context_id.as_str(), &name).await
            {
                Ok(()) => {
                    tracing::info!("Renamed conversation {} to {}", context_id.as_ref(), name);
                    if sender.send(Message::ConversationsRefresh).is_err() {
                        tracing::debug!("Message receiver dropped - UI may be shutting down");
                    }
                },
                Err(e) => {
                    tracing::error!("Failed to rename conversation: {}", e);
                },
            }
        });
    }

    pub(crate) async fn spawn_delete_conversation(&self, context_id: ContextId) {
        let api_url = self.api_external_url.clone();
        let token = self.admin_token.clone();
        let sender = self.message_tx.clone();
        let current_ctx = self.current_context_id.read().await.clone();

        tokio::spawn(async move {
            match cloud_api::delete_context(&api_url, &token, context_id.as_str()).await {
                Ok(()) => {
                    tracing::info!("Deleted conversation: {}", context_id.as_ref());
                    if current_ctx.as_ref() == context_id.as_ref()
                        && sender
                            .send(Message::ConversationDeleted(context_id.to_string()))
                            .is_err()
                    {
                        tracing::debug!("Message receiver dropped - UI may be shutting down");
                    }
                    if sender.send(Message::ConversationsRefresh).is_err() {
                        tracing::debug!("Message receiver dropped - UI may be shutting down");
                    }
                },
                Err(e) => {
                    tracing::error!("Failed to delete conversation: {}", e);
                },
            }
        });
    }

    pub(crate) fn spawn_create_conversation(&self, name: String) {
        let api_url = self.api_external_url.clone();
        let token = self.admin_token.clone();
        let sender = self.message_tx.clone();
        tokio::spawn(async move {
            match cloud_api::create_context_with_name(&api_url, &token, &name).await {
                Ok(new_context_id) => {
                    tracing::info!("Created new conversation: {}", new_context_id);
                    if sender.send(Message::ConversationsRefresh).is_err() {
                        tracing::debug!("Message receiver dropped - UI may be shutting down");
                    }
                },
                Err(e) => {
                    tracing::error!("Failed to create conversation: {}", e);
                },
            }
        });
    }
}
