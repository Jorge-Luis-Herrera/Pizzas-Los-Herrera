//! Biblioteca de la API de Pizzería Los Herrera.
//!
//! Se separa de `main.rs` para que los tests de integración puedan montar el
//! router real y ejercitarlo, en lugar de duplicar la lógica.

pub mod db;
pub mod handlers;
pub mod models;
pub mod routes;
pub mod state;
