use std::pin::Pin;

use futures::Future;
// Export the Language Server Protocol types.
pub use lsp_types;
pub use tracing;
pub use async_trait;
pub use futures;
pub use event_listener;

pub mod futures_extensions;
pub mod fs;

/// An owned dynamically dispatched `Future`.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + Sync + 'a>>;

// Unit test fixtures.
#[cfg(test)]
mod tests;
