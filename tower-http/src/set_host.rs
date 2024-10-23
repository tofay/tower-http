//! Middleware to set the HOST header on requests.
//!
//! # Example
//!
//! ```
//! use tower_http::set_host::SetHostLayer;
//! use http::header::HOST;
//! use http::HeaderValue;
//! use http::{Request, Response};
//! use bytes::Bytes;
//! use http_body_util::{BodyExt, Full};
//! use std::convert::Infallible;
//! use tower::{ServiceBuilder, Service, ServiceExt};
//!
//! async fn handle(mut req: Request<()>) -> Result<Response<Full<Bytes>>, Infallible> {
//!     let host_header = req.headers_mut().remove(HOST).unwrap();
//!     Ok(Response::new(Full::new(Bytes::from(host_header.as_bytes().to_vec()))))
//! }
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let mut service = ServiceBuilder::new()
//!     .layer(SetHostLayer::new())
//!     .service_fn(handle);
//!
//! // Call the service.
//! let request = Request::builder().uri("https://rust-lang.org/").body(())?;
//! let response = service.ready().await?.call(request).await?;
//! let data = response.into_body().collect().await?.to_bytes();
//! assert_eq!(&data[..], b"rust-lang.org");
//! #
//! # Ok(())
//! # }
//! ```

use http::{header::HOST, HeaderValue, Request, Response, Uri};
use std::{
    fmt,
    task::{Context, Poll},
};
use tower::Service;
use tower_layer::Layer;

fn is_schema_secure(uri: &Uri) -> bool {
    uri.scheme_str()
        .map(|scheme_str| matches!(scheme_str, "wss" | "https"))
        .unwrap_or_default()
}

fn get_non_default_port(uri: &Uri) -> Option<http::uri::Port<&str>> {
    match (uri.port().map(|p| p.as_u16()), is_schema_secure(uri)) {
        (Some(443), true) => None,
        (Some(80), false) => None,
        _ => uri.port(),
    }
}

/// Layer that adds the `Host` header on requests if it is not present.
#[derive(Debug, Clone)]
pub struct SetHostLayer {}

impl SetHostLayer {
    /// Create a new [`SetHostLayer`].
    pub fn new() -> Self {
        SetHostLayer {}
    }
}

impl Default for SetHostLayer {
    fn default() -> Self {
        Self::new()
    }
}

/// Middleware to set the HOST header on requests if it is not present.
pub struct SetHost<S> {
    inner: S,
}

impl<S> SetHost<S> {
    define_inner_service_accessors!();
}

impl<S> Layer<S> for SetHostLayer {
    type Service = SetHost<S>;

    fn layer(&self, inner: S) -> Self::Service {
        SetHost { inner }
    }
}

impl<S> fmt::Debug for SetHost<S>
where
    S: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SetHost")
            .field("inner", &self.inner)
            .finish()
    }
}

impl<ReqBody, ResBody, S> Service<Request<ReqBody>> for SetHost<S>
where
    S: Service<Request<ReqBody>, Response = Response<ResBody>>,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;

    #[inline]
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut req: Request<ReqBody>) -> Self::Future {
        let uri = req.uri().clone();
        req.headers_mut().entry(HOST).or_insert_with(|| {
            let hostname = uri.host().expect("authority implies host");
            if let Some(port) = get_non_default_port(&uri) {
                let s = format!("{}:{}", hostname, port);
                HeaderValue::from_str(&s)
            } else {
                HeaderValue::from_str(hostname)
            }
            .expect("uri host is valid header value")
        });
        self.inner.call(req)
    }
}
