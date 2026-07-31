use tokio::{
    sync::mpsc,
    task::{AbortHandle, JoinHandle},
};

use super::{
    runtime::{WorkerCommand, send_event},
    types::{HttpMethod, HttpRequest, HttpResponse, NetworkEvent},
};

pub(super) fn spawn_http_request(
    client: reqwest::Client,
    request: HttpRequest,
    event_tx: mpsc::UnboundedSender<NetworkEvent>,
    command_tx: mpsc::UnboundedSender<WorkerCommand>,
    generation: u64,
) -> AbortHandle {
    let request_id = request.request_id;
    let task: JoinHandle<()> = tokio::spawn(async move {
        let event = execute_http_request(client, request).await;
        send_event(&event_tx, event);
        let _ = command_tx.send(WorkerCommand::HttpFinished {
            request_id,
            generation,
        });
    });
    task.abort_handle()
}

async fn execute_http_request(client: reqwest::Client, request: HttpRequest) -> NetworkEvent {
    let request_id = request.request_id;
    let method = match reqwest_method(&request.method) {
        Ok(method) => method,
        Err(error) => {
            return NetworkEvent::HttpError { request_id, error };
        }
    };

    let mut builder = client.request(method, request.url).timeout(request.timeout);

    for (name, value) in request.headers {
        builder = builder.header(name, value);
    }

    if let Some(body) = request.body {
        builder = builder.body(body);
    }

    let result = async {
        let mut response = builder.send().await.map_err(|error| error.to_string())?;
        let status = response.status().as_u16();
        let headers = response
            .headers()
            .iter()
            .map(|(name, value)| {
                (
                    name.as_str().to_string(),
                    value.to_str().unwrap_or_default().to_string(),
                )
            })
            .collect::<Vec<_>>();
        if let Some(max_response_bytes) = request.max_response_bytes
            && response
                .content_length()
                .is_some_and(|bytes| bytes > max_response_bytes as u64)
        {
            return Err(format!(
                "HTTP_RESPONSE_BODY_LIMIT_EXCEEDED:{max_response_bytes}"
            ));
        }

        let mut body = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|error| error.to_string())? {
            let next_len = body.len().saturating_add(chunk.len());
            if request
                .max_response_bytes
                .is_some_and(|limit| next_len > limit)
            {
                return Err(format!(
                    "HTTP_RESPONSE_BODY_LIMIT_EXCEEDED:{}",
                    request.max_response_bytes.unwrap_or_default()
                ));
            }
            body.extend_from_slice(&chunk);
        }

        Ok::<_, String>(HttpResponse {
            request_id,
            status,
            headers,
            body,
        })
    }
    .await;

    match result {
        Ok(response) => NetworkEvent::HttpResponse(response),
        Err(error) => NetworkEvent::HttpError { request_id, error },
    }
}

fn reqwest_method(method: &HttpMethod) -> Result<reqwest::Method, String> {
    match method {
        HttpMethod::Get => Ok(reqwest::Method::GET),
        HttpMethod::Post => Ok(reqwest::Method::POST),
        HttpMethod::Put => Ok(reqwest::Method::PUT),
        HttpMethod::Patch => Ok(reqwest::Method::PATCH),
        HttpMethod::Delete => Ok(reqwest::Method::DELETE),
        HttpMethod::Head => Ok(reqwest::Method::HEAD),
        HttpMethod::Options => Ok(reqwest::Method::OPTIONS),
        HttpMethod::Custom(value) => reqwest::Method::from_bytes(value.as_bytes())
            .map_err(|err| format!("invalid HTTP method `{value}`: {err}")),
    }
}
