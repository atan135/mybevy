use tokio::sync::mpsc;

use super::{
    runtime::send_event,
    types::{HttpMethod, HttpRequest, HttpResponse, NetworkEvent},
};

pub(super) fn spawn_http_request(
    client: reqwest::Client,
    request: HttpRequest,
    event_tx: mpsc::UnboundedSender<NetworkEvent>,
) {
    tokio::spawn(async move {
        let event = execute_http_request(client, request).await;
        send_event(&event_tx, event);
    });
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
        let response = builder.send().await?;
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
        let body = response.bytes().await?.to_vec();

        Ok::<_, reqwest::Error>(HttpResponse {
            request_id,
            status,
            headers,
            body,
        })
    }
    .await;

    match result {
        Ok(response) => NetworkEvent::HttpResponse(response),
        Err(err) => NetworkEvent::HttpError {
            request_id,
            error: err.to_string(),
        },
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
