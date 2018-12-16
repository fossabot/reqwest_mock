use body::Body;
use client::stub::{StubClient, StubRequest, StubResponse};
use client::stub::error::RegisterStubError;
use reqwest::header::{IntoHeaderName, HeaderValue, HeaderMap};
use reqwest::{Method, StatusCode, Url};

/// A request stub builder to be used in conjunction with `StubClient`.
///
/// After you are finished specifying the details of the matching request, call `response()` to
/// return a `ResponseStubber` instance and start specifying the response. Finally use
/// `ResponseStubber::mock()` to register the mock into the client.
#[must_use]
pub struct RequestStubber<'cl> {
    client: &'cl mut StubClient,
    url: Url,

    _method: Option<Method>,
    _body: Option<Body>,
    _headers: Option<HeaderMap>,
}

impl<'cl> RequestStubber<'cl> {
    pub(super) fn new(client: &'cl mut StubClient, url: Url) -> RequestStubber<'cl> {
        RequestStubber {
            client: client,
            url: url,
            _method: None,
            _body: None,
            _headers: None,
        }
    }

    /// Set the method of the request.
    pub fn method(mut self, method: Method) -> Self {
        self._method = Some(method);
        self
    }

    /// Set the body of the request.
    pub fn body<B: Into<Body>>(mut self, body: B) -> Self {
        self._body = Some(body.into());
        self
    }

    /// Add a header to the request.
    pub fn header<HN: IntoHeaderName>(mut self, name: HN, value: HeaderValue) -> Self {
        self._headers = Some(self._headers.map_or_else(HeaderMap::new, |mut hs| {
            hs.insert(name, value);
            hs
        }));
        self
    }

    /// Add multiple headers to the request.
    pub fn headers(mut self, headers: HeaderMap) -> Self {
        self._headers = Some(self._headers.map_or_else(HeaderMap::new, |mut hs| {
            hs.extend(headers);
            hs
        }));
        self
    }

    /// Stub the response to this request.
    pub fn response(self) -> ResponseStubber<'cl> {
        ResponseStubber {
            client: self.client,
            req: StubRequest {
                url: self.url,
                method: self._method,
                body: self._body,
                headers: self._headers.map(|hs| ::helper::serialize_headers(&hs)),
            },

            _status_code: StatusCode::OK,
            _body: None,
            _headers: HeaderMap::new(),
        }
    }
}

/// A response stub builder to be used in conjunction with `StubClient`.
#[must_use]
pub struct ResponseStubber<'cl> {
    client: &'cl mut StubClient,
    req: StubRequest,

    _status_code: StatusCode,
    _body: Option<Body>,
    _headers: HeaderMap,
}

impl<'cl> ResponseStubber<'cl> {
    /// Set the status code of the response.
    pub fn status_code(mut self, status: StatusCode) -> Self {
        self._status_code = status;
        self
    }

    /// Set the body of the response.
    pub fn body<B: Into<Body>>(mut self, body: B) -> Self {
        self._body = Some(body.into());
        self
    }

    /// Add a header to the response.
    pub fn header<HN: IntoHeaderName>(mut self, name: HN, value: HeaderValue) -> Self {
        self._headers.insert(name, value);
        self
    }

    /// Add multiple headers to the response.
    pub fn headers(mut self, headers: HeaderMap) -> Self {
        self._headers.extend(headers);
        self
    }

    /// Register the mock in the client.
    pub fn mock(self) -> Result<(), RegisterStubError> {
        let resp = StubResponse {
            status_code: self._status_code,
            body: self._body,
            headers: self._headers,
        };
        self.client.register_stub(
            self.req
                .try_to_key()
                .map_err(|e| RegisterStubError::ReadFile(e))?,
            resp,
        )
    }
}
