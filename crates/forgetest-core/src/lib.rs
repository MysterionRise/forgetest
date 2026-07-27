//! forgetest-core — Core eval engine, traits, and scoring.
//!
//! This crate defines the fundamental data model, traits, and scoring logic
//! that the entire forgetest system builds on.

pub mod agent;
pub mod engine;
pub mod error;
pub mod harbor;
pub mod model;
pub mod parser;
pub mod report;
pub mod repository_engine;
pub mod repository_report;
pub mod results;
pub mod statistics;
pub mod suite;
pub mod traits;
