//! JSON/MessagePack wire formats: request decoding, `Accept` negotiation and response encoding.

use axum::{
    body::Body,
    extract::MatchedPath,
    http::{HeaderMap, HeaderValue, Request, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
use serde::{Serialize, de::DeserializeOwned};
use std::io::Cursor;

use super::error::{ApiError, HttpRejection};
use crate::domain::core::error::DomainError;

const CONTENT_TYPE_JSON: &str = "application/json";
const CONTENT_TYPE_MSGPACK: &str = "application/vnd.msgpack";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum BodyFormat {
    Json,
    MsgPack,
}

tokio::task_local! {
    /// Negotiation encloses the timeout too, so synthetic errors use the same encoding.
    static RESPONSE_FORMAT: BodyFormat;
}

impl BodyFormat {
    pub(super) fn content_type(self) -> &'static str {
        match self {
            Self::Json => CONTENT_TYPE_JSON,
            Self::MsgPack => CONTENT_TYPE_MSGPACK,
        }
    }

    /// Format negotiated for the current request; JSON outside the negotiation scope.
    pub(super) fn negotiated() -> Self {
        RESPONSE_FORMAT.try_with(|format| *format).unwrap_or(Self::Json)
    }

    /// Requires exactly one supported `Content-Type`. MessagePack input remains accepted when
    /// response negotiation is disabled.
    pub(super) fn of_request(headers: &HeaderMap) -> Result<Self, ApiError> {
        let mut values = headers.get_all(header::CONTENT_TYPE).iter();
        let value = values
            .next()
            .and_then(|value| value.to_str().ok())
            .ok_or(HttpRejection::UnsupportedMediaType)?;
        if values.next().is_some() {
            return Err(HttpRejection::UnsupportedMediaType.into());
        }

        let media_type = value.split(';').next().unwrap_or_default().trim();
        if media_type.eq_ignore_ascii_case(CONTENT_TYPE_JSON) {
            Ok(Self::Json)
        } else if media_type.eq_ignore_ascii_case(CONTENT_TYPE_MSGPACK)
            || media_type.eq_ignore_ascii_case("application/msgpack")
            || media_type.eq_ignore_ascii_case("application/x-msgpack")
        {
            Ok(Self::MsgPack)
        } else {
            Err(HttpRejection::UnsupportedMediaType.into())
        }
    }

    /// Decodes one document; rejections never carry deserializer messages or input values.
    pub(super) fn decode<T: DeserializeOwned>(self, bytes: &[u8]) -> Result<T, ApiError> {
        match self {
            Self::Json => serde_json::from_slice(bytes).map_err(|error| {
                let rejection = match error.classify() {
                    serde_json::error::Category::Data => HttpRejection::BodyContract,
                    serde_json::error::Category::Io => HttpRejection::BodyRead,
                    serde_json::error::Category::Syntax | serde_json::error::Category::Eof => {
                        HttpRejection::InvalidBody
                    }
                };
                ApiError::from(rejection)
            }),
            Self::MsgPack => {
                let mut decoder = rmp_serde::Deserializer::new(Cursor::new(bytes));
                decoder.set_max_depth(128);
                let value = T::deserialize(&mut decoder).map_err(|error| {
                    let rejection = match error {
                        // 0xc1 is reserved by MessagePack: it is malformed input, not a DTO mismatch.
                        rmp_serde::decode::Error::TypeMismatch(marker)
                            if marker.to_u8() == 0xc1 =>
                        {
                            HttpRejection::InvalidBody
                        }
                        rmp_serde::decode::Error::TypeMismatch(_)
                        | rmp_serde::decode::Error::OutOfRange
                        | rmp_serde::decode::Error::LengthMismatch(_)
                        | rmp_serde::decode::Error::Syntax(_) => HttpRejection::BodyContract,
                        _ => HttpRejection::InvalidBody,
                    };
                    ApiError::from(rejection)
                })?;

                // A body is one document, not a stream of concatenated MessagePack values.
                let length = u64::try_from(bytes.len()).map_err(|_| {
                    DomainError::internal("Request body length cannot be represented as u64")
                })?;
                if decoder.position() != length {
                    return Err(HttpRejection::InvalidBody.into());
                }
                Ok(value)
            }
        }
    }

    /// Encoder errors may describe the value, so only the format name is returned.
    pub(super) fn encode<T: Serialize>(self, value: &T) -> Result<Vec<u8>, &'static str> {
        match self {
            Self::Json => serde_json::to_vec(value).map_err(|_| "JSON"),
            Self::MsgPack => rmp_serde::to_vec_named(value).map_err(|_| "MessagePack"),
        }
    }
}

/// Responses are serialized once inside this task-local scope, including timeout/rejection errors.
pub(super) async fn negotiation(req: Request<Body>, next: Next) -> Response {
    // Probes have no negotiated representation; Accept must not turn a plain status into 406.
    if req
        .extensions()
        .get::<MatchedPath>()
        .is_some_and(|path| matches!(path.as_str(), "/healthz" | "/readyz"))
    {
        return next.run(req).await;
    }
    let mut response = match accepted_format(req.headers()) {
        Ok(format) => RESPONSE_FORMAT.scope(format, next.run(req)).await,
        Err(error) => {
            RESPONSE_FORMAT.scope(BodyFormat::Json, async { error.into_response() }).await
        }
    };
    vary_accept(response.headers_mut());
    response
}

fn vary_accept(headers: &mut HeaderMap) {
    let already_varies = headers.get_all(header::VARY).iter().any(|value| {
        value.to_str().is_ok_and(|value| {
            value.split(',').any(|token| {
                let token = token.trim();
                token == "*" || token.eq_ignore_ascii_case("accept")
            })
        })
    });
    if !already_varies {
        headers.append(header::VARY, HeaderValue::from_static("Accept"));
    }
}

#[derive(Clone, Copy)]
struct AcceptPreference {
    specificity: u8,
    quality: u16,
}

fn accepted_format(headers: &HeaderMap) -> Result<BodyFormat, ApiError> {
    if !headers.contains_key(header::ACCEPT) {
        return Ok(BodyFormat::Json);
    }

    let mut json = None;
    let mut msgpack = None;
    for value in headers.get_all(header::ACCEPT) {
        let value = value.to_str().map_err(|_| ApiError::from(HttpRejection::NotAcceptable))?;
        for item in value.split(',') {
            let mut parameters = item.split(';');
            let media_type = parameters.next().unwrap_or_default().trim();
            let quality = parse_parameters(parameters).unwrap_or(0);
            if media_type.eq_ignore_ascii_case(CONTENT_TYPE_JSON) {
                update_preference(&mut json, AcceptPreference { specificity: 2, quality });
            } else if media_type.eq_ignore_ascii_case(CONTENT_TYPE_MSGPACK) {
                update_preference(&mut msgpack, AcceptPreference { specificity: 2, quality });
            } else if media_type.eq_ignore_ascii_case("application/*") || media_type == "*/*" {
                let specificity = if media_type == "*/*" { 0 } else { 1 };
                let preference = AcceptPreference { specificity, quality };
                update_preference(&mut json, preference);
                update_preference(&mut msgpack, preference);
            }
        }
    }

    let json = json.filter(|preference| preference.quality > 0);
    let msgpack = msgpack.filter(|preference| preference.quality > 0);
    match (json, msgpack) {
        (None, None) => Err(HttpRejection::NotAcceptable.into()),
        (Some(json), Some(msgpack))
            if (msgpack.quality, msgpack.specificity) > (json.quality, json.specificity) =>
        {
            Ok(BodyFormat::MsgPack)
        }
        (None, Some(_)) => Ok(BodyFormat::MsgPack),
        (Some(_), _) => Ok(BodyFormat::Json),
    }
}

/// Returns the media range quality; malformed parameters or a repeated `q` exclude the range.
fn parse_parameters<'a>(parameters: impl Iterator<Item = &'a str>) -> Option<u16> {
    let mut quality = None;
    for parameter in parameters {
        let (name, value) = parameter.trim().split_once('=')?;
        if name.trim().eq_ignore_ascii_case("q") {
            if quality.is_some() {
                return None;
            }
            quality = Some(parse_quality(value.trim())?);
        }
    }
    Some(quality.unwrap_or(1000))
}

fn update_preference(current: &mut Option<AcceptPreference>, candidate: AcceptPreference) {
    if current.is_none_or(|current| {
        (candidate.specificity, candidate.quality) > (current.specificity, current.quality)
    }) {
        *current = Some(candidate);
    }
}

/// RFC 9110 qvalues have at most three decimals. Integer thousandths avoid float rounding and
/// distinguish exclusions (`q=0`) from positive weights such as `q=0.01`.
fn parse_quality(value: &str) -> Option<u16> {
    let (whole, fraction) = value.split_once('.').unwrap_or((value, ""));
    if fraction.len() > 3 || !fraction.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    match whole {
        "1" if fraction.bytes().all(|byte| byte == b'0') => Some(1000),
        "0" => fraction.bytes().zip([100_u16, 10, 1]).try_fold(0_u16, |quality, (digit, place)| {
            quality.checked_add(u16::from(digit.checked_sub(b'0')?).checked_mul(place)?)
        }),
        _ => None,
    }
}
