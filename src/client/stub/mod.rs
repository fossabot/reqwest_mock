use body::Body;
use client::Client;
use config::ClientConfig;
use error::Error;
use request::{Request, RequestHeader};
use reqwest::header::HeaderMap;
use reqwest::{Method, StatusCode, Url};
use response::Response;
use std::collections::{BTreeMap, HashMap};

mod settings;
pub use self::settings::{StubDefault, StubSettings, StubStrictness};

mod builder;
pub use self::builder::{RequestStubber, ResponseStubber};

mod error;
pub use self::error::RegisterStubError;
// TODO should not be public
pub use self::error::FieldError;

#[derive(Hash, PartialEq, Eq)]
struct StubKey {
    url: Url,
    method: Option<Method>,
    body: Option<Vec<u8>>,
    headers: Option<BTreeMap<String, String>>,
}

struct StubRequest {
    url: Url,
    method: Option<Method>,
    body: Option<Body>,
    headers: Option<BTreeMap<String, String>>,
}

impl StubRequest {
    fn try_to_key(self) -> Result<StubKey, ::std::io::Error> {
        Ok(StubKey {
            url: self.url,
            method: self.method,
            body: match self.body {
                Some(b) => Some(b.try_to_vec()?),
                None => None,
            },
            headers: self.headers,
        })
    }
}

struct StubResponse {
    status_code: StatusCode,
    body: Option<Body>,
    headers: HeaderMap,
}

/// A client which allows you to stub out the response to a request explicitly.
///
/// # Examples
/// ```
/// use reqwest_mock::{Client, Method, StubClient, StubDefault, StubSettings, StubStrictness, Url};
///
/// let mut client = StubClient::new(StubSettings {
///     // If a request without a corresponding stub is made we want an error
///     // to be returned when our code executes the request.
///     default: StubDefault::Error,
///
///     // We want the `StubClient` to compare actual requests and provided
///     // mocks by their method and their url.
///     strictness: StubStrictness::MethodUrl,
/// });
///
/// // Mock a request.
/// client
///     .stub(Url::parse("http://example.com/mocking").unwrap())
///         .method(Method::GET)
///     .response()
///         .body("Mocking is fun!")
///         .mock();
///
/// let response = client.get("http://example.com/mocking").send().unwrap();
/// assert_eq!(response.body_to_utf8().unwrap(), "Mocking is fun!".to_string());
/// ```
pub struct StubClient {
    config: ClientConfig,
    stubs: HashMap<StubKey, Response>,
    settings: StubSettings,
}

impl StubClient {
    /// Create a new instance of `StubClient`.
    ///
    /// Please consult [StubSettings](struct.StubSettings.html) for more information about the
    /// possible settings.
    pub fn new(stub_settings: StubSettings) -> Self {
        StubClient {
            config: ClientConfig::default(),
            stubs: HashMap::new(),
            settings: stub_settings,
        }
    }

    /// Provide a stub for a request to the provided url.
    ///
    /// This will return a [RequestStubber](struct.RequestStubber.html), which in a first step will
    /// allow you to specify the full details of the request. Make sure that they match the
    /// [StubStrictness](struct.StubStrictness.html) provided in the settings.
    ///
    /// After you are finished specifying the details of the matching request, call `response()` to
    /// return a `ResponseStubber` instance and start specifying the response. Finally use
    /// `ResponseStubber::mock()` to register the mock into the client.
    pub fn stub<'cl>(&'cl mut self, url: Url) -> RequestStubber<'cl> {
        RequestStubber::new(self, url)
    }

    /// Return the appropriate `StubKey` for the provided request.
    fn stub_key(&self, header: &RequestHeader, body: &Option<Vec<u8>>) -> StubKey {
        match self.settings.strictness {
            StubStrictness::Full => StubKey {
                url: header.url.clone(),
                method: Some(header.method.clone()),
                body: body.clone(),
                headers: Some(::helper::serialize_headers(&header.headers)),
            },
            StubStrictness::BodyMethodUrl => StubKey {
                url: header.url.clone(),
                method: Some(header.method.clone()),
                body: body.clone(),
                headers: None,
            },
            StubStrictness::HeadersMethodUrl => StubKey {
                url: header.url.clone(),
                method: Some(header.method.clone()),
                body: None,
                headers: Some(::helper::serialize_headers(&header.headers)),
            },
            StubStrictness::MethodUrl => StubKey {
                url: header.url.clone(),
                method: Some(header.method.clone()),
                body: None,
                headers: None,
            },
            StubStrictness::Url => StubKey {
                url: header.url.clone(),
                method: None,
                body: None,
                headers: None,
            },
        }
    }

    pub(self) fn register_stub(
        &mut self,
        key: StubKey,
        value: StubResponse,
    ) -> Result<(), RegisterStubError> {
        // Check if stub key contains the nescessary fields.
        macro_rules! validate_sk_field {
            (Some $field:ident $strictness:path) => (
                if key.$field.is_none() {
                    return Err(RegisterStubError::MissingField(FieldError {
                        field_name: stringify!($field),
                        strictness: stringify!($strictness)
                    }));
                }
            );
            (None $field:ident $strictness:path) => (
                if key.$field.is_some() {
                    return Err(RegisterStubError::UnescessaryField(FieldError {
                        field_name: stringify!($field),
                        strictness: stringify!($strictness)
                    }));
                }
            );
        }

        macro_rules! validate_sk_fields {
            (  $strictness:path; $($sn:tt $field:ident),* )
            => ( $( validate_sk_field!($sn $field $strictness); )* )
        }

        match self.settings.strictness {
            StubStrictness::Full => {
                validate_sk_fields!(StubStrictness::Full; Some method, Some body, Some headers);
            }
            StubStrictness::BodyMethodUrl => {
                validate_sk_fields!(StubStrictness::BodyMethodUrl; Some method, Some body, None headers);
            }
            StubStrictness::HeadersMethodUrl => {
                validate_sk_fields!(StubStrictness::HeadersMethodUrl; Some method, None body, Some headers);
            }
            StubStrictness::MethodUrl => {
                validate_sk_fields!(StubStrictness::MethodUrl; Some method, None body, None headers);
            }
            StubStrictness::Url => {
                validate_sk_fields!(StubStrictness::Url; None method, None body, None headers);
            }
        }

        // Register the response.
        let response = Response {
            url: key.url.clone(),
            status: value.status_code,
            headers: value.headers,
            body: value
                .body
                .map(|b| b.try_to_vec())
                .unwrap_or_else(|| Ok(Vec::new()))
                .map_err(|e| RegisterStubError::ReadFile(e))?,
        };
        self.stubs.insert(key, response);
        Ok(())
    }
}

impl Client for StubClient {
    fn execute(&self, config: Option<&ClientConfig>, request: Request) -> Result<Response, Error> {
        // Check if there is a recorded stub for the request.
        let header = request.header;
        let body = match request.body {
            Some(b) => Some(b.try_to_vec()?),
            None => None,
        };

        let key = self.stub_key(&header, &body);
        match self.stubs.get(&key) {
            Some(resp) => Ok(resp.clone()),
            None => {
                match self.settings.default {
                    StubDefault::Panic => {
                        // TODO provide more diagonistics using log crate.
                        panic!(
                            "Requested {}, without having provided a stub for it.",
                            header.url
                        );
                    }
                    StubDefault::Error => {
                        // TODO provide more diagonistics using log crate.
                        Err(format!(
                            "Requested {}, without having provided a stub for it.",
                            header.url
                        ).into())
                    }
                    StubDefault::PerformRequest => {
                        use client::DirectClient;
                        let client = DirectClient::new();
                        let request = Request {
                            header: header,
                            body: body.map(Body::from),
                        };
                        client.execute(config, request)
                    }
                }
            }
        }
    }

    fn config(&self) -> &ClientConfig {
        &self.config
    }

    fn config_mut(&mut self) -> &mut ClientConfig {
        &mut self.config
    }
}
