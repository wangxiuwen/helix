//! Helix application modules — organized by functional domain.

// ============================================================================
// Module groups
// ============================================================================

pub mod infra;     // config, logger, database, security, api_server, etc.
pub mod app;       // tray, scheduler, cron, update_checker, cloudflared
pub mod agent;     // AI agent, tools, skills, hooks, commands, memory, plugins
pub mod ai;        // providers, streaming, model_selection, ai_chat
pub mod chat;      // channels, sessions, messaging
pub mod cloud;     // kubeconfig, aliyun
pub mod browser;   // browser engine
pub mod evomap;    // EvoMap

// ============================================================================
// Backward-compatible re-exports
// ============================================================================
// These allow `crate::modules::config`, `crate::modules::database`, etc.
// to keep working without changing every callsite.

// infra
pub use infra::config;
pub use infra::logger;
pub use infra::log_bridge;
pub use infra::database;
pub use infra::security;
pub use infra::notifications;
pub use infra::i18n;
pub use infra::api_server;

// app
pub use app::tray;
pub use app::scheduler;
pub use app::cron;
pub use app::update_checker;
pub use app::cloudflared;
pub use app::workspace;
pub use app::environments;
pub use app::mcp;

// agent (core re-exported via agent/mod.rs `pub use core::*`)
pub use agent::tools as agent_tools;
pub use agent::subagents;
pub use agent::skills;
pub use agent::hooks;
pub use agent::commands;
pub use agent::memory;
#[allow(unused_imports)]
pub use agent::sandbox;
pub use agent::plugins;

// ai
pub use ai::chat as ai_chat;
pub use ai::providers;
pub use ai::streaming;
pub use ai::model_selection;
pub use ai::stream_events;
pub use ai::usage;
pub use ai::link_understanding;
pub use ai::media_understanding;

// chat
pub use chat::channels;
pub use chat::sessions;
pub use chat::messaging;

// cloud
pub use cloud::kubeconfig;
pub use cloud::aliyun;

// browser
pub use browser::engine as browser_engine;

// Top-level re-exports from config
pub use config::*;
#[allow(unused_imports)]
pub use logger::*;
