use mongodb::bson::oid::ObjectId;
use mongodb::error::{Error, ErrorKind, WriteFailure};

use crate::domain::core::error::{DomainError, DomainResult, StatusCode};

pub(super) fn parse_oid(
    raw: &str,
    param: &'static str,
    entity: &'static str,
) -> DomainResult<ObjectId> {
    ObjectId::parse_str(raw).map_err(|_| {
        DomainError::new(
            StatusCode::BAD_REQUEST,
            format!("Invalid {entity} ObjectId on parameter `{param}`"),
        )
        .with_public(format!("Invalid {param}: Invalid {entity} identifier"))
        .with_trace_code("MONGO_INVALID_OBJECT_ID")
    })
}

// Driver messages and details can contain query or document values. Only static
// categories and numeric codes may cross this adapter's error boundary.
pub(super) fn map(error: Error, entity: &'static str) -> DomainError {
    // Connectivity failures are transient (503); everything else is an internal fault (500).
    let (category, code, transient) = match error.kind.as_ref() {
        ErrorKind::Write(WriteFailure::WriteError(write_error)) if write_error.code == 11000 => {
            return duplicate(entity);
        }
        ErrorKind::Command(command_error) if command_error.code == 11000 => {
            return duplicate(entity);
        }
        ErrorKind::Command(command_error) => ("command", Some(command_error.code), false),
        ErrorKind::Write(WriteFailure::WriteError(write_error)) => {
            ("write", Some(write_error.code), false)
        }
        ErrorKind::Write(WriteFailure::WriteConcernError(concern_error)) => {
            ("write concern", Some(concern_error.code), false)
        }
        ErrorKind::Write(_) => ("write", None, false),
        ErrorKind::InsertMany(_) | ErrorKind::BulkWrite(_) => ("bulk write", None, false),
        ErrorKind::Bson(_) => ("BSON conversion", None, false),
        ErrorKind::Io(_) => ("I/O", None, true),
        ErrorKind::DnsResolve { .. } => ("DNS resolution", None, true),
        ErrorKind::Authentication { .. } => ("authentication", None, false),
        ErrorKind::ConnectionPoolCleared { .. } => ("connection pool", None, true),
        ErrorKind::ServerSelection { .. } => ("server selection", None, true),
        ErrorKind::InvalidResponse { .. } => ("invalid response", None, false),
        ErrorKind::InvalidArgument { .. } => ("invalid argument", None, false),
        ErrorKind::InvalidTlsConfig { .. } => ("TLS configuration", None, false),
        ErrorKind::IncompatibleServer { .. } => ("server compatibility", None, false),
        ErrorKind::Transaction { .. } => ("transaction", None, false),
        ErrorKind::Internal { .. } => ("internal driver", None, false),
        ErrorKind::Shutdown => ("client shutdown", None, true),
        _ => ("driver", None, false),
    };

    let detail = match code {
        Some(code) => format!("MongoDB {category} failure on {entity} (code {code})"),
        None => format!("MongoDB {category} failure on {entity}"),
    };
    if transient {
        DomainError::new(StatusCode::SERVICE_UNAVAILABLE, detail)
            .with_trace_code("MONGO_TRANSIENT_FAILURE")
    } else {
        DomainError::new(StatusCode::INTERNAL_SERVER_ERROR, detail)
            .with_trace_code("MONGO_DRIVER_FAILURE")
    }
}

fn duplicate(entity: &'static str) -> DomainError {
    DomainError::new(StatusCode::CONFLICT, format!("MongoDB duplicate key on {entity} (E11000)"))
        .with_public(format!("{entity} already exists: unique key is already in use"))
        .with_trace_code("MONGO_DUPLICATE_KEY")
}
