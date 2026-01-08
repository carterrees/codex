use codex_council::client::CouncilClient;
use codex_council::prompts;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn test_gemini_client_integration() {
    // Start a mock server
    let server = MockServer::start().await;

    // Configure environment to point to the mock server
    unsafe {
        std::env::set_var("GEMINI_BASE_URL", format!("{}/", server.uri()));
        std::env::set_var("GEMINI_API_KEY", "fake-key");
    }

    // Expect a POST request to /chat/completions (since Gemini is configured as WireApi::Chat)
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .and(header("Authorization", "Bearer fake-key"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"Hello from Mock Gemini\"}}]}\n\ndata: [DONE]\n"
        ))
        .mount(&server)
        .await;

    // Create the client
    let client = CouncilClient::new(prompts::MODEL_CRITIC_GEMINI)
        .await
        .expect("Failed to create client");

    // Send a message
    let response = client
        .send_message("System Prompt".to_string(), "User Message".to_string())
        .await
        .expect("Failed to send message");

    // Verify response
    assert_eq!(response, "Hello from Mock Gemini");

    // Verify request payload via the mock server
    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1);
    let body: serde_json::Value = serde_json::from_slice(&requests[0].body).unwrap();
    
    assert_eq!(body["model"], prompts::MODEL_CRITIC_GEMINI);
    assert_eq!(body["messages"][0]["role"], "system");
    assert_eq!(body["messages"][0]["content"], "System Prompt");
    assert_eq!(body["messages"][1]["role"], "user");
    assert_eq!(body["messages"][1]["content"], "User Message");
}
