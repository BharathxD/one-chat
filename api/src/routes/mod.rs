// This file makes modules within src/routes/ accessible.
// For example, if you have src/routes/thread_routes.rs:

pub mod thread_routes;
pub mod message_routes;
pub mod share_routes;
pub mod health_routes;
pub mod attachment_routes;
pub mod voice_routes;
pub mod openai_compatible_routes;
// pub mod user_routes; // etc.
