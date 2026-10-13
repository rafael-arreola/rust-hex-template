use mongodb::error::{Error, ErrorKind, WriteFailure};

use crate::domain::core::error::DomainError;

// Driver messages and details can contain query or document values. Only static
// categories and numeric codes may cross this adapter's error boundary.
pub(super) fn map(error: Error, entity: &'static str) -> DomainError {
    // Connectivity failures are transient (503); everything else is an internal fault (500).
    let (category, code, transient) = match error.kind.as_ref() {
        ErrorKind::Write(WriteFailure::WriteError(write_error)) if write_error.code == 11000 => {
            return DomainError::duplicate(entity, "unique key");
        }
        ErrorKind::Command(command_error) if command_error.code == 11000 => {
            return DomainError::duplicate(entity, "unique key");
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
        Some(code) => format!("{category} failure on {entity} (code {code})"),
        None => format!("{category} failure on {entity}"),
    };
    if transient {
        DomainError::service_unavailable("MongoDB", detail)
    } else {
        DomainError::internal(format!("MongoDB {detail}"))
    }
}
