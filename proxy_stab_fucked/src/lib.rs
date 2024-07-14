//! Stabilis package
//!
//! This package contains the components that are used for the STAB module of the Stabilis protocol.
//! The STAB module is responsible for the creation of the STAB token, and managing its stability.
//!
//! The package consists of the following components:
//! - `stabilis_component`: The main component of the STAB module, which is responsible for the creation and management of the STAB token. It holds all state necessary to manage this.
//! - `proxy`: Interacting with the Stabilis component goes through the proxy component. This is used to:
//!     - Update the Stabilis component with new parameters / data, such as:
//!         - The interest rate
//!         - Collateral prices
//!     - Ensure that the Stabilis component is only interacted with by authorized callers.
//!     - Ensure potential upgrades to the Stabilis component can be done without disrupting the rest of the system.
//! - `flash_loans`: The flash loans component, which allows users to borrow STAB tokens from the Stabilis component.
//! - `stabilis_liquidity_pool`: The liquidity pool component, which is a STAB/XRD liquidity pool native to the Stabilis protocol. It is used to determine the price of STAB tokens.
//!
//! More information on each component can be found in their respective modules.

mod flash_loans;
mod proxy;
mod shared_structs;
mod stabilis_component;
mod stabilis_liquidity_pool;
