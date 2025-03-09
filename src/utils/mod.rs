//! # Utils Module
//!
//! The `utils` module provides utility functions and structures that support the core functionality
//! of the tastytrade library. It simplifies common tasks and provides essential infrastructure
//! for application configuration and logging.
//!
//! ## Overview
//!
//! This module contains several components that handle configuration management and logging:
//!
//! - **Configuration Management**: Handles application settings including API credentials,
//!   environment selection (demo/production), and logging preferences.
//! - **Logging System**: Provides a robust logging infrastructure with configurable log levels
//!   and safe initialization logic.
//!
//! ## Submodules
//!
//! ### Config (`config`)
//!
//! The `config` module manages the application's configuration settings, providing methods to:
//!
//! - Load configuration from environment variables
//! - Read/write configuration from JSON files
//! - Validate credentials
//! - Create TastyTrade client instances with the appropriate settings
//!
//! Configuration options include:
//! - API credentials (username and password)
//! - Environment selection (demo/production)
//! - Log level configuration
//! - Session management settings
//!
//! **Example:**
//! ```rust,no_run
//! use tastytrade::utils::config::Config;
//!
//! // Initialize configuration from environment variables
//! let config = Config::from_env();
//!
//! // Create a TastyTrade client
//! let tasty = config.create_client();
//! ```
//!
//! ### Logger (`logger`)
//!
//! The `logger` module provides a logging system built on the `tracing` crate. Features include:
//!
//! - Thread-safe, idempotent logger initialization
//! - Configurable log levels (DEBUG, INFO, WARN, ERROR, TRACE)
//! - Environment-variable based configuration
//! - Support for multiple platforms (with special handling for wasm32)
//!
//! **Log Levels:**
//! - `DEBUG`: Detailed debugging information
//! - `INFO`: General application status information
//! - `WARN`: Non-critical issues that require attention
//! - `ERROR`: Significant problems causing failures
//! - `TRACE`: Fine-grained application execution details
//!
//!
//! ## Usage Notes
//!
//! - The configuration system automatically initializes the logger when loading settings
//! - Logger initialization is thread-safe and only happens once
//! - Configuration can be loaded from environment variables or JSON files
//! - Credentials can be securely stored (passwords are excluded from serialization)

/// This module contains the configuration
pub mod config;

/// and logger setup for the application.
pub mod logger;
