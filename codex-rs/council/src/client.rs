use anyhow::Result;
use codex_api::ChatClient;
use codex_api::ChatRequestBuilder;
use codex_api::Provider;
use codex_api::ReqwestTransport;
use codex_core::default_client::build_reqwest_client;
use codex_core::model_provider_info::ModelProviderInfo;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;

#[derive(Clone)]
pub struct SimpleAuthProvider {
    api_key: String,
}

impl codex_api::auth::AuthProvider for SimpleAuthProvider {
    fn bearer_token(&self) -> Option<String> {
        Some(self.api_key.clone())
    }
}

pub struct CouncilClient {
    pub model_id: String,
    pub client: ChatClient<ReqwestTransport, SimpleAuthProvider>,
    pub provider: Provider,
}

impl CouncilClient {
    pub async fn new(model_id: &str) -> Result<Self> {
        let provider_info = if model_id.contains("gemini") {
            ModelProviderInfo::create_gemini_provider()
        } else {
            ModelProviderInfo::create_openai_provider()
        };

        let api_provider = provider_info.to_api_provider(None)?;
        let api_key = provider_info.api_key()?.unwrap_or_default();
        let auth = SimpleAuthProvider { api_key };

        let transport = ReqwestTransport::new(build_reqwest_client());
        let client = ChatClient::new(transport, api_provider.clone(), auth);

        Ok(Self {
            model_id: model_id.to_string(),
            client,
            provider: api_provider,
        })
    }

    pub async fn send_message(
        &self,
        system_prompt: String,
        user_message: String,
    ) -> Result<String> {
        let input = vec![ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText { text: user_message }],
        }];

        let request = ChatRequestBuilder::new(&self.model_id, &system_prompt, &input, &[])
            .build(&self.provider)?;

        // Send request
        let mut stream = self.client.stream_request(request).await?;
        let mut full_content = String::new();

        use codex_api::ResponseEvent;
        use futures::StreamExt;

        while let Some(event) = stream.next().await {
            match event? {
                ResponseEvent::OutputTextDelta(delta) => {
                    full_content.push_str(&delta);
                }
                ResponseEvent::OutputItemDone(ResponseItem::Message { content, role, .. }) => {
                    if role == "assistant" && full_content.is_empty() {
                        for c in content {
                            if let ContentItem::OutputText { text } = c {
                                full_content.push_str(&text);
                                break;
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        if full_content.is_empty() {
            let model_id = &self.model_id;
            anyhow::bail!("No content in response from {model_id}");
        }

        Ok(full_content)
    }
}
