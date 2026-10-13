use futures::future::BoxFuture;
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware, Next};
use reqwest_tracing::{DefaultSpanBackend, TracingMiddleware};
use std::ops::Deref;
use std::time::Duration;

pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);
pub const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Clone)]
pub struct HttpClient {
    inner: ClientWithMiddleware,
}

impl HttpClient {
    pub fn try_new() -> Result<Self, reqwest::Error> {
        let client = reqwest::Client::builder()
            .timeout(DEFAULT_TIMEOUT)
            .connect_timeout(DEFAULT_CONNECT_TIMEOUT)
            .user_agent(env!("CARGO_PKG_NAME"))
            .redirect(reqwest::redirect::Policy::none())
            .build()?;

        let sanitize_errors: for<'a> fn(
            reqwest::Request,
            &'a mut _,
            Next<'a>,
        ) -> BoxFuture<
            'a,
            reqwest_middleware::Result<reqwest::Response>,
        > = |request, extensions, next| {
            Box::pin(async move {
                next.run(request, extensions).await.map_err(reqwest_middleware::Error::without_url)
            })
        };

        let inner = ClientBuilder::new(client)
            .with(TracingMiddleware::<DefaultSpanBackend>::new())
            .with(sanitize_errors)
            .build();
        Ok(Self { inner })
    }

    pub fn inner(&self) -> &ClientWithMiddleware {
        &self.inner
    }
}

impl Deref for HttpClient {
    type Target = ClientWithMiddleware;

    fn deref(&self) -> &Self::Target {
        self.inner()
    }
}
