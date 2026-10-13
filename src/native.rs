//! Register only the product helpers used by compiled application loaders.
use axonyx_runtime::native::{AxNativeError, AxNativeRegistry};

pub fn register(registry: &mut AxNativeRegistry) -> Result<(), AxNativeError> {
    legura::pagination::register(registry)
}
