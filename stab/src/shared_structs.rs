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
    pub date_liquidated: Instant,
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
pub struct StabPriceData {
    /// The latest price errors for the STAB token (market price - internal price), used for calculating the interest rate
    pub latest_stab_price_errors: KeyValueStore<u64, Decimal>,
    /// The total of the latest price errors
    pub latest_stab_price_errors_total: Decimal,
    /// The time of the last update
    pub last_update: Instant,
    /// The key of the last price change in the price_errors KVS
    pub last_changed_price: u64,
    /// STAB token internal price
    pub internal_price: Decimal,
    /// Whether the cache is full
    pub full_cache: bool,
    /// The interest rate for the STAB token
    pub interest_rate: Decimal,
}

#[derive(ScryptoSbor)]
pub struct InterestParameters {
    /// The Kp value for the interest rate calculation
    pub kp: Decimal,
    /// The Ki value for the interest rate calculation
    pub ki: Decimal,
    /// The maximum interest rate
    pub max_interest_rate: Decimal,
    /// The minimum interest rate
    pub min_interest_rate: Decimal,
    /// The allowed deviation for the internal price (for it to not count any price error in interest rate calculation)
    pub allowed_deviation: Decimal,
    /// The maximum price error allowed
    pub max_price_error: Decimal,
    /// The offset for the price error
    pub price_error_offset: Decimal,
}
