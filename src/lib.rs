// Copyright (c) 2025 Whatsmeow-rs Contributors
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! # whatsmeow-rs
//!
//! A Rust client library for the WhatsApp Web multidevice API.
//!
//! This is a port of the Go library [whatsmeow](https://github.com/tulir/whatsmeow)
//! to Rust, providing async/await support and Rust ecosystem integration.

pub mod auth;
pub mod binary;
pub mod client;
pub mod error;
pub mod messaging;
pub mod proto;
pub mod socket;
pub mod store;
pub mod types;
pub mod util;

pub use client::Client;
pub use error::{Error, Result};
pub use types::*;

/// Version information
pub const VERSION: &str = env!("CARGO_PKG_VERSION");