use scrypto::prelude::*;

#[derive(ScryptoSbor, NonFungibleData)]
pub struct Cdp {
    pub collateral: ResourceAddress,
    pub parent_address: ResourceAddress,
    pub is_pool_unit_collateral: bool,

    #[mutable]
    pub collateral_amount: Decimal,
    #[mutable]
    pub minted_stab: Decimal,
    #[mutable]
    pub collateral_stab_ratio: Decimal,
    #[mutable]
    pub status: CdpStatus,
    #[mutable]
    pub marker_id: u64,
}

#[derive(ScryptoSbor, NonFungibleData)]
pub struct CdpMarker {
    pub mark_type: CdpUpdate,
    pub time_marked: Instant,
    pub marked_id: NonFungibleLocalId,
    pub marker_placing: Decimal,

    #[mutable]
    pub used: bool,
}

#[derive(ScryptoSbor, NonFungibleData)]
pub struct LiquidationReceipt {
    pub collateral: ResourceAddress,
    pub stab_paid: Decimal,
    pub percentage_received: Decimal,
    pub percentage_owed: Decimal,
    pub cdp_liquidated: NonFungibleLocalId,
}

#[derive(ScryptoSbor, PartialEq)]
pub enum CdpStatus {
    Healthy,
    Marked,
    Liquidated,
    ForceLiquidated,
    Closed,
}

#[derive(ScryptoSbor, PartialEq)]
pub enum CdpUpdate {
    Marked,
    Saved,
}

#[derive(ScryptoSbor)]
pub struct CollateralInfo {
    pub mcr: Decimal,
    pub usd_price: Decimal,
    pub liquidation_collateral_ratio: Decimal,
    pub vault: Vault,
    pub resource_address: ResourceAddress,
    pub treasury: Vault,
    pub accepted: bool,
    pub initialized: bool,
    pub max_stab_share: Decimal,
    pub minted_stab: Decimal,
    pub highest_cr: Decimal,
}

#[derive(ScryptoSbor)]
pub struct PoolUnitInfo {
    pub vault: Vault,
    pub treasury: Vault,
    pub lsu: bool,
    pub validator: Option<Global<Validator>>,
    pub one_resource_pool: Option<Global<OneResourcePool>>,
    pub parent_address: ResourceAddress,
    pub address: ResourceAddress,
    pub accepted: bool,
    pub minted_stab: Decimal,
    pub max_pool_share: Decimal,
}

#[derive(ScryptoSbor)]
pub struct ProtocolParameters {
    pub minimum_mint: Decimal,
    pub max_vector_length: u64,
    pub liquidation_delay: i64,
    pub unmarked_delay: i64,
    pub liquidation_liquidation_fine: Decimal,
    pub stabilis_liquidation_fine: Decimal,
    pub stop_liquidations: bool,
    pub stop_openings: bool,
    pub stop_closings: bool,
    pub stop_force_mint: bool,
    pub stop_force_liquidate: bool,
    pub force_mint_cr_multiplier: Decimal,
}
