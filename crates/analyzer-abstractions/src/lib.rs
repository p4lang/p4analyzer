use std::pin::Pin;

use futures::Future;

// Export the Language Server Protocol types.
pub use async_trait;
pub use event_listener;
pub use futures;
pub use lsp_types;
pub use tracing;

pub mod fs;
pub mod futures_extensions;

/// An owned dynamically dispatched `Future`.

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + Sync + 'a>>;

// Unit test fixtures.
#[cfg(test)]
mod tests;
