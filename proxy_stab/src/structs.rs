use scrypto::prelude::*;

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
pub struct LoanReceipt {
    #[mutable]
    pub borrowed_amount: Decimal,
    pub interest: Decimal,
}