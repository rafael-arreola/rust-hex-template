use std::collections::{HashMap, HashSet};
use std::fmt;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use jsonwebtoken::{
    Algorithm, DecodingKey, DecodingKeyKind, Validation, decode, decode_header, jwk,
};
use reqwest::{Url, header};
use serde::de::{self, DeserializeOwned, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer};
use serde_json::{Map, Number, Value};
use tokio::{
    sync::{Mutex, RwLock, mpsc, watch},
    task::JoinHandle,
};

use super::MAX_CREDENTIAL_BYTES;
use crate::domain::core::error::{DomainError, DomainResult, StatusCode};
use crate::shared::http_client::HttpClient;

const CACHE_TTL: Duration = Duration::from_secs(15 * 60);
const REFRESH_COOLDOWN: Duration = Duration::from_secs(30);
const MAX_JWKS_BYTES: usize = 1024 * 1024;
const MAX_JWKS_KEYS: usize = 64;
const MAX_KID_BYTES: usize = 128;
const MIN_RSA_MODULUS_BYTES: usize = 256;
// jsonwebtoken 11.1 rust_crypto constructs rsa::RsaPublicKey with these backend limits.
const MAX_RSA_MODULUS_BYTES: usize = 512;
const MAX_RSA_EXPONENT_BYTES: usize = 5;
const MAX_RSA_EXPONENT: u64 = 8_589_934_591;

/// Registered claims stay in the adapter; jsonwebtoken checks their signature and semantics first.
#[derive(Deserialize)]
pub(super) struct RegisteredClaims {
    pub(super) sub: String,
    pub(super) exp: u64,
    #[serde(default)]
    pub(super) iss: Option<String>,
    #[serde(default)]
    aud: Option<Audience>,
    #[serde(default, deserialize_with = "deserialize_not_before")]
    nbf: Option<u64>,
}

impl RegisteredClaims {
    pub(super) fn validate_shape(&self, require_iss: bool, require_aud: bool) -> DomainResult<()> {
        let valid_iss = match &self.iss {
            Some(issuer) => !issuer.trim().is_empty(),
            None => !require_iss,
        };
        let valid_aud = match &self.aud {
            Some(Audience::Single(audience)) => !audience.trim().is_empty(),
            Some(Audience::Multiple(audiences)) => {
                (!require_aud || !audiences.is_empty())
                    && audiences.iter().all(|audience| !audience.trim().is_empty())
            }
            None => !require_aud,
        };
        if self.sub.trim().is_empty()
            || self.exp == 0
            || !valid_iss
            || !valid_aud
            || self.nbf.is_some_and(|not_before| not_before > self.exp)
        {
            return Err(DomainError::new(
                StatusCode::UNAUTHORIZED,
                "Invalid registered JWT claims",
            )
            .with_trace_code("JWT_REGISTERED_CLAIMS_INVALID"));
        }
        Ok(())
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Audience {
    Single(String),
    Multiple(Vec<String>),
}

// Absence is optional; an explicitly present null/non-numeric nbf is not a NumericDate.
fn deserialize_not_before<'de, D>(deserializer: D) -> Result<Option<u64>, D::Error>
where
    D: Deserializer<'de>,
{
    u64::deserialize(deserializer).map(Some)
}

struct KeyCache {
    keys: HashMap<String, DecodingKey>,
    fetched_at: Instant,
}

impl KeyCache {
    fn is_fresh(&self) -> bool {
        self.fetched_at.elapsed() < CACHE_TTL
    }
}

#[derive(Clone, Copy)]
struct JwksFailure(&'static str);

impl From<JwksFailure> for DomainError {
    fn from(failure: JwksFailure) -> Self {
        unavailable(failure.0)
    }
}

#[derive(Clone)]
struct RefreshCompletion {
    generation: u64,
    completed_at: Instant,
    result: Result<Arc<KeyCache>, JwksFailure>,
}

#[derive(Clone)]
struct RefreshFlight {
    generation: u64,
    completion: watch::Receiver<Option<RefreshCompletion>>,
}

impl RefreshFlight {
    fn reusable(&self) -> bool {
        self.completion
            .borrow()
            .as_ref()
            .is_none_or(|completion| completion.completed_at.elapsed() < REFRESH_COOLDOWN)
    }
}

struct RefreshRequest {
    generation: u64,
    completion: watch::Sender<Option<RefreshCompletion>>,
}

#[derive(Default)]
struct RefreshState {
    generation: u64,
    // No flight after bootstrap deliberately permits the first unknown-kid rotation lookup.
    flight: Option<RefreshFlight>,
}

pub(super) struct JwksConfig {
    pub(super) jwks_url: String,
    pub(super) issuer: Option<String>,
    pub(super) audiences: Vec<String>,
    pub(super) algorithm: Algorithm,
}

pub(super) struct JwksVerifier {
    validation: Validation,
    cache: Arc<RwLock<Arc<KeyCache>>>,
    refresh: Mutex<RefreshState>,
    refresh_requests: mpsc::Sender<RefreshRequest>,
    worker: JoinHandle<()>,
}

impl Drop for JwksVerifier {
    fn drop(&mut self) {
        // The worker never owns the verifier, so the last provider owner can stop it without a cycle.
        self.worker.abort();
    }
}

impl JwksVerifier {
    #[tracing::instrument(skip_all, fields(dependency = "jwks"))]
    pub(super) async fn new(config: JwksConfig, http: HttpClient) -> DomainResult<Self> {
        let jwks_url = validate_config(&config)?;
        let mut validation = Validation::new(config.algorithm);
        let mut required_claims = vec!["exp", "sub"];
        if let Some(issuer) = &config.issuer {
            required_claims.push("iss");
            validation.set_issuer(&[issuer]);
        }
        if !config.audiences.is_empty() {
            required_claims.push("aud");
            validation.set_audience(&config.audiences);
            validation.validate_aud = true;
        } else {
            validation.validate_aud = false;
        }
        validation.set_required_spec_claims(&required_claims);
        validation.validate_exp = true;
        validation.validate_nbf = true;
        validation.leeway = 0;
        validation.reject_tokens_expiring_in_less_than = 1;

        let keys = Self::fetch_keys(&http, &jwks_url).await?;
        let cache = Arc::new(RwLock::new(Arc::new(KeyCache { keys, fetched_at: Instant::now() })));
        let (refresh_requests, requests) = mpsc::channel(1);
        // One bootstrap task per verifier; request cancellation only drops an awaiting receiver.
        let worker =
            tokio::spawn(Self::refresh_worker(http, jwks_url, Arc::clone(&cache), requests));
        Ok(Self {
            validation,
            cache,
            refresh: Mutex::new(RefreshState::default()),
            refresh_requests,
            worker,
        })
    }

    pub(super) async fn verify<T>(&self, credential: &str) -> DomainResult<(RegisteredClaims, T)>
    where
        T: DeserializeOwned,
    {
        if credential.is_empty()
            || credential.len() > MAX_CREDENTIAL_BYTES
            || !credential
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
            || credential.split('.').count() != 3
            || credential.split('.').any(str::is_empty)
        {
            return Err(DomainError::new(
                StatusCode::UNAUTHORIZED,
                "Malformed or oversized JWT credential",
            )
            .with_trace_code("JWT_CREDENTIAL_MALFORMED"));
        }
        let header = decode_header(credential).map_err(|_| {
            DomainError::new(StatusCode::UNAUTHORIZED, "Malformed JWT header")
                .with_trace_code("JWT_HEADER_MALFORMED")
        })?;
        if !self.validation.algorithms.contains(&header.alg) || header.crit.is_some() {
            return Err(DomainError::new(
                StatusCode::UNAUTHORIZED,
                "Unsupported JWT algorithm or critical header",
            )
            .with_trace_code("JWT_ALGORITHM_UNSUPPORTED"));
        }
        let kid = header.kid.filter(|kid| valid_kid(kid)).ok_or_else(|| {
            DomainError::new(StatusCode::UNAUTHORIZED, "JWT requires a bounded, non-empty key ID")
                .with_trace_code("JWT_KID_INVALID")
        })?;

        // The configured endpoint is the only key source; jku, x5u and embedded jwk are ignored.
        let key = self.key_for(&kid).await?;
        let verified =
            decode::<ClaimDocument>(credential, &key, &self.validation).map_err(|_| {
                DomainError::new(
                    StatusCode::UNAUTHORIZED,
                    "JWT signature or claims validation failed",
                )
                .with_trace_code("JWT_SIGNATURE_OR_CLAIMS_INVALID")
            })?;
        // Both projections read the full verified document, so X may also declare registered claims.
        let registered = RegisteredClaims::deserialize(&verified.claims.0).map_err(|_| {
            DomainError::new(StatusCode::UNAUTHORIZED, "Invalid registered JWT claims")
                .with_trace_code("JWT_REGISTERED_CLAIMS_INVALID")
        })?;
        registered.validate_shape(self.validation.iss.is_some(), self.validation.validate_aud)?;
        let claims = T::deserialize(&verified.claims.0).map_err(|_| {
            DomainError::new(StatusCode::UNAUTHORIZED, "Invalid provider JWT claims")
                .with_trace_code("JWT_PROVIDER_CLAIMS_INVALID")
        })?;
        Ok((registered, claims))
    }

    async fn cached_key(&self, kid: &str) -> Option<DecodingKey> {
        let cache = self.cache.read().await;
        if cache.is_fresh() { cache.keys.get(kid).cloned() } else { None }
    }

    async fn key_for(&self, kid: &str) -> DomainResult<DecodingKey> {
        if let Some(key) = self.cached_key(kid).await {
            return Ok(key);
        }

        let mut flight = {
            // Coordination is short-lived; neither network I/O nor completion waits hold this lock.
            let mut refresh = self.refresh.lock().await;
            if let Some(key) = self.cached_key(kid).await {
                return Ok(key);
            }
            match refresh.flight.as_ref() {
                Some(flight) if flight.reusable() => flight.clone(),
                _ => {
                    let generation = refresh.generation.checked_add(1).ok_or_else(|| {
                        DomainError::new(
                            StatusCode::INTERNAL_SERVER_ERROR,
                            "JWKS refresh generation counter exhausted",
                        )
                        .with_trace_code("JWKS_GENERATION_EXHAUSTED")
                    })?;
                    let (completion, receiver) = watch::channel(None);
                    let flight = RefreshFlight { generation, completion: receiver };
                    self.refresh_requests
                        .try_send(RefreshRequest { generation, completion })
                        .map_err(|_| unavailable("JWKS refresh worker is unavailable"))?;
                    refresh.generation = generation;
                    refresh.flight = Some(flight.clone());
                    flight
                }
            }
        };

        loop {
            // Drop the watch borrow before awaiting; every waiter reads this flight's own snapshot.
            let completion = flight.completion.borrow_and_update().clone();
            if let Some(completion) = completion {
                if completion.generation != flight.generation {
                    return Err(DomainError::new(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "JWKS refresh generation mismatch",
                    )
                    .with_trace_code("JWKS_GENERATION_MISMATCH"));
                }
                let cache = completion.result?;
                if !cache.is_fresh() {
                    return Err(unavailable("JWKS refresh result expired before it could be used"));
                }
                return cache.keys.get(kid).cloned().ok_or_else(|| {
                    DomainError::new(
                        StatusCode::UNAUTHORIZED,
                        "JWT key ID is not in the refreshed JWKS",
                    )
                    .with_trace_code("JWKS_KID_NOT_FOUND")
                });
            }
            flight.completion.changed().await.map_err(|_| {
                unavailable("JWKS refresh worker stopped before completing the refresh")
            })?;
        }
    }

    #[tracing::instrument(skip_all, fields(dependency = "jwks"))]
    async fn refresh_worker(
        http: HttpClient,
        url: Url,
        cache: Arc<RwLock<Arc<KeyCache>>>,
        mut requests: mpsc::Receiver<RefreshRequest>,
    ) {
        while let Some(request) = requests.recv().await {
            let result = match Self::fetch_keys(&http, &url).await {
                Ok(keys) => {
                    let replacement = Arc::new(KeyCache { keys, fetched_at: Instant::now() });
                    *cache.write().await = Arc::clone(&replacement);
                    Ok(replacement)
                }
                Err(failure) => Err(failure),
            };
            let completion = RefreshCompletion {
                generation: request.generation,
                completed_at: Instant::now(),
                result,
            };
            request.completion.send_modify(|value| *value = Some(completion));
        }
    }

    #[tracing::instrument(skip_all, fields(dependency = "jwks"))]
    async fn fetch_keys(
        http: &HttpClient,
        url: &Url,
    ) -> Result<HashMap<String, DecodingKey>, JwksFailure> {
        let mut response = http
            .inner()
            .get(url.clone())
            .header(header::ACCEPT, "application/json")
            .send()
            .await
            .map_err(|_| JwksFailure("JWKS request failed"))?;
        if response.status() != reqwest::StatusCode::OK {
            return Err(JwksFailure("JWKS endpoint returned an unsuccessful response"));
        }
        if response.content_length().is_some_and(|length| {
            usize::try_from(length).map_or(true, |length| length > MAX_JWKS_BYTES)
        }) {
            return Err(JwksFailure("JWKS response exceeds its size limit"));
        }

        let mut body = Vec::new();
        while let Some(chunk) =
            response.chunk().await.map_err(|_| JwksFailure("JWKS response could not be read"))?
        {
            if chunk.len() > MAX_JWKS_BYTES.saturating_sub(body.len()) {
                return Err(JwksFailure("JWKS response exceeds its size limit"));
            }
            body.extend_from_slice(&chunk);
        }
        let jwks: jwk::JwkSet =
            serde_json::from_slice(&body).map_err(|_| JwksFailure("JWKS response is malformed"))?;
        if jwks.keys.is_empty() || jwks.keys.len() > MAX_JWKS_KEYS {
            return Err(JwksFailure("JWKS key count is outside its allowed bounds"));
        }

        let mut keys = HashMap::new();
        let mut identifiers = HashSet::new();
        for jwk in &jwks.keys {
            let Some(kid) = &jwk.common.key_id else {
                continue;
            };
            if !valid_kid(kid) || !identifiers.insert(kid.clone()) {
                return Err(JwksFailure("JWKS contains an invalid or duplicate key ID"));
            }
            if !matches!(jwk.algorithm, jwk::AlgorithmParameters::RSA(_))
                || jwk
                    .common
                    .public_key_use
                    .as_ref()
                    .is_some_and(|usage| *usage != jwk::PublicKeyUse::Signature)
                || jwk
                    .common
                    .key_algorithm
                    .is_some_and(|algorithm| algorithm != jwk::KeyAlgorithm::RS256)
                || jwk
                    .common
                    .key_operations
                    .as_ref()
                    .is_some_and(|operations| !operations.contains(&jwk::KeyOperations::Verify))
            {
                continue;
            }
            let key = DecodingKey::try_from(jwk)
                .map_err(|_| JwksFailure("JWKS contains malformed RSA key material"))?;
            validate_rsa_key(&key)?;
            keys.insert(kid.clone(), key);
        }
        if keys.is_empty() {
            return Err(JwksFailure("JWKS contains no usable RS256 verification key"));
        }
        Ok(keys)
    }
}

fn valid_kid(kid: &str) -> bool {
    !kid.is_empty()
        && kid.len() <= MAX_KID_BYTES
        && kid.trim() == kid
        && !kid.chars().any(char::is_control)
}

fn validate_config(config: &JwksConfig) -> DomainResult<Url> {
    if config.algorithm != Algorithm::RS256 {
        return Err(DomainError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "JWT verifier currently supports only RS256",
        )
        .with_trace_code("JWKS_CONFIG_ALGORITHM_UNSUPPORTED"));
    }
    if let Some(issuer) = &config.issuer
        && (issuer.trim().is_empty() || issuer.trim() != issuer)
    {
        return Err(DomainError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "JWT configuration requires an exact non-empty issuer",
        )
        .with_trace_code("JWKS_CONFIG_ISSUER_INVALID"));
    }
    if config
        .audiences
        .iter()
        .any(|audience| audience.trim().is_empty() || audience.trim() != audience)
    {
        return Err(DomainError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "JWT configuration contains an invalid API audience",
        )
        .with_trace_code("JWKS_CONFIG_AUDIENCE_INVALID"));
    }

    let url = Url::parse(&config.jwks_url).map_err(|_| {
        DomainError::new(StatusCode::INTERNAL_SERVER_ERROR, "JWKS endpoint is not a valid URL")
            .with_trace_code("JWKS_CONFIG_URL_INVALID")
    })?;
    if !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
        || url.query().is_some()
        || url.host_str().is_none()
    {
        return Err(DomainError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "JWKS endpoint must not contain credentials, query or fragment",
        )
        .with_trace_code("JWKS_CONFIG_URL_FORBIDDEN_PARTS"));
    }
    let loopback = url.host_str().is_some_and(|host| {
        host.eq_ignore_ascii_case("localhost")
            || host
                .trim_start_matches('[')
                .trim_end_matches(']')
                .parse::<IpAddr>()
                .is_ok_and(|address| address.is_loopback())
    });
    if url.scheme() != "https" && !(url.scheme() == "http" && loopback) {
        return Err(DomainError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "JWKS endpoint requires HTTPS outside loopback",
        )
        .with_trace_code("JWKS_CONFIG_HTTPS_REQUIRED"));
    }
    Ok(url)
}

fn validate_rsa_key(key: &DecodingKey) -> Result<(), JwksFailure> {
    let DecodingKeyKind::RsaModulusExponent { n, e } = key.kind() else {
        return Err(JwksFailure("JWKS verification key is not an RSA public key"));
    };
    let modulus_has_enough_bits = n.len() > MIN_RSA_MODULUS_BYTES
        || (n.len() == MIN_RSA_MODULUS_BYTES && n.first().is_some_and(|byte| *byte >= 128));
    if !modulus_has_enough_bits
        || n.len() > MAX_RSA_MODULUS_BYTES
        || n.first().is_none_or(|byte| *byte == 0)
        || n.last().is_none_or(|byte| (*byte & 1) == 0)
        || e.is_empty()
        || e.len() > MAX_RSA_EXPONENT_BYTES
    {
        return Err(JwksFailure("JWKS RSA parameters are outside their allowed bounds"));
    }
    let exponent = e
        .iter()
        .try_fold(0_u64, |value, byte| {
            value.checked_mul(256).and_then(|value| value.checked_add(u64::from(*byte)))
        })
        .ok_or(JwksFailure("JWKS RSA exponent exceeds its size limit"))?;
    // Together with the >=2048-bit modulus, this also guarantees the backend's e < n check.
    if !(3..=MAX_RSA_EXPONENT).contains(&exponent) || (exponent & 1) == 0 {
        return Err(JwksFailure("JWKS RSA exponent is outside the cryptographic backend's bounds"));
    }
    Ok(())
}

fn unavailable(detail: &'static str) -> DomainError {
    DomainError::new(StatusCode::SERVICE_UNAVAILABLE, format!("JWKS: {detail}"))
        .with_trace_code("JWKS_UNAVAILABLE")
}

/// Preserves JSON claims while rejecting duplicate keys in every object.
struct ClaimDocument(Value);

impl<'de> Deserialize<'de> for ClaimDocument {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(ClaimDocumentVisitor)
    }
}

struct ClaimDocumentVisitor;

impl<'de> Visitor<'de> for ClaimDocumentVisitor {
    type Value = ClaimDocument;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a JSON claim document")
    }

    fn visit_bool<E: de::Error>(self, value: bool) -> Result<Self::Value, E> {
        Ok(ClaimDocument(Value::Bool(value)))
    }

    fn visit_i64<E: de::Error>(self, value: i64) -> Result<Self::Value, E> {
        Ok(ClaimDocument(Value::Number(Number::from(value))))
    }

    fn visit_u64<E: de::Error>(self, value: u64) -> Result<Self::Value, E> {
        Ok(ClaimDocument(Value::Number(Number::from(value))))
    }

    fn visit_f64<E: de::Error>(self, value: f64) -> Result<Self::Value, E> {
        Number::from_f64(value)
            .map(|number| ClaimDocument(Value::Number(number)))
            .ok_or_else(|| E::custom("JWT claims contain a non-finite number"))
    }

    fn visit_str<E: de::Error>(self, value: &str) -> Result<Self::Value, E> {
        Ok(ClaimDocument(Value::String(value.to_owned())))
    }

    fn visit_string<E: de::Error>(self, value: String) -> Result<Self::Value, E> {
        Ok(ClaimDocument(Value::String(value)))
    }

    fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
        Ok(ClaimDocument(Value::Null))
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<Self::Value, A::Error> {
        let mut values = Vec::new();
        while let Some(ClaimDocument(value)) = sequence.next_element::<ClaimDocument>()? {
            values.push(value);
        }
        Ok(ClaimDocument(Value::Array(values)))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut object: A) -> Result<Self::Value, A::Error> {
        let mut values = Map::new();
        while let Some(key) = object.next_key::<String>()? {
            if values.contains_key(&key) {
                return Err(de::Error::custom("JWT claims contain a duplicate object key"));
            }
            let ClaimDocument(value) = object.next_value::<ClaimDocument>()?;
            values.insert(key, value);
        }
        Ok(ClaimDocument(Value::Object(values)))
    }
}
