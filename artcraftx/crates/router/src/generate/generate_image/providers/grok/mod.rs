//! First-party (cookie-session) Grok Imagine image generation.
//!
//! Sends prompts on the user's own grok.com "imagine" websocket (the app owns
//! the socket; see `RouterGrokClient`). Sending returns the prompt's request
//! id immediately; finished images arrive later on the same socket, keyed by
//! that id. Fast vs quality ("pro") is a flag on the request.
pub mod grok_imagine_image;
