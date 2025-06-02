//! Tiny alias so we can return `Result<T>` everywhere.

pub type Result<T> = std::result::Result<T, anyhow::Error>;
