use scrypto::prelude::*;

// NFT receipt structure, minted when an unstake is requested, redeemable after a set delay.
#[derive(ScryptoSbor, NonFungibleData)]
pub struct UnstakeReceipt {
    #[mutable]
    pub address: ResourceAddress,
    #[mutable]
    pub amount: Decimal,
    #[mutable]
    pub redemption_time: Instant,
}

// Staking ID structure, holding staked and locked amounts and date until which they are locked. Also stores the next period to claim rewards (updated after a user has claimed them).
#[derive(ScryptoSbor, NonFungibleData)]
pub struct Id {
    #[mutable]
    pub resources: HashMap<ResourceAddress, Resource>,
    #[mutable]
    pub next_period: i64,
}

// Lock structure, holding the information about locking options of a token.
#[derive(ScryptoSbor)]
pub struct Lock {
    pub payment: Decimal,
    pub duration: i64,
}

#[derive(ScryptoSbor, Clone)]
pub struct Resource {
    pub amount_staked: Decimal,
    pub locked_until: Option<Instant>,
}

// Stakable unit structure, used by the component to data about a stakable token.
#[derive(ScryptoSbor)]
pub struct StakableUnit {
    pub address: ResourceAddress,
    pub amount_staked: Decimal,
    pub vault: Vault,
    pub reward_amount: Decimal,
    pub lock: Lock,
    pub rewards: KeyValueStore<i64, Decimal>,
}

// Stake transfer receipt structure, minted when a user wants to transfer their staked tokens, redeemable by other users to add these tokens to their own staking ID.
#[derive(ScryptoSbor, NonFungibleData)]
pub struct StakeTransferReceipt {
    pub address: ResourceAddress,
    pub amount: Decimal,
}

#[derive(ScryptoSbor)]
pub struct Proposal {
    pub title: String,
    pub description: String,
    pub steps: Vec<ProposalStep>,
    pub votes_for: Decimal,
    pub votes_against: Decimal,
    pub votes: KeyValueStore<NonFungibleLocalId, Decimal>,
    pub deadline: Instant,
    pub next_index: i64,
    pub status: ProposalStatus,
}

#[derive(ScryptoSbor, NonFungibleData)]
pub struct ProposalReceipt {
    #[mutable]
    pub fee_paid: Decimal,
    #[mutable]
    pub proposal_id: u64,
    #[mutable]
    pub status: ProposalStatus,
}

#[derive(ScryptoSbor)]
pub struct ProposalStep {
    pub component: ComponentAddress,
    pub badge: ResourceAddress,
    pub method: String,
    pub args: ScryptoValue,
}

#[derive(ScryptoSbor, PartialEq)]
pub enum ProposalStatus {
    Building,
    Ongoing,
    Rejected,
    Accepted,
    Executed,
    Finished,
}

#[derive(ScryptoSbor)]
pub struct GovernanceParameters {
    pub fee: Decimal,
    pub proposal_duration: i64,
    pub quorum: Decimal,
    pub minimum_for_fraction: Decimal,
}