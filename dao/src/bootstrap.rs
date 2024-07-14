//! # Linear Bootstrap Pool Blueprint
//! 
//! Blueprint can be used to create a Balancer style linear bootstrap pool, where the weights of the pool change linearly over time.
//! This can be used to distribute tokens in a fair way, while only needed a small initial (liquidity) investment.

use scrypto::prelude::*;

#[blueprint]
mod bootstrap {
    enable_method_auth! {
        methods {
            remove_liquidity => PUBLIC;
            get_resource1_price => PUBLIC;
            swap => PUBLIC;
            finish_bootstrap => restrict_to: [OWNER];
            reclaim_initial => PUBLIC;
        }
    }

    extern_blueprint! {
        "package_tdx_2_1phmkv5tql452y7eev899qngwesfzjn2zdjdd2efh50e73rtq93ne0q",
        BasicPool {
            fn instantiate_with_liquidity(a_bucket: Bucket, b_bucket: Bucket, input_fee_rate: Decimal, dapp_definition: ComponentAddress) -> (Global<BasicPool>, Bucket);
        }
    }

    struct LinearBootstrapPool {
        /// The TwoResourcePool component that holds both sides of the pool
        pool_component: Global<TwoResourcePool>,
        /// Fee to be paid on swaps
        fee: Decimal,
        /// Initial weight of the first resource
        initial_weight1: Decimal,
        /// Initial weight of the second resource
        initial_weight2: Decimal,
        /// Target weight of the first resource
        target_weight1: Decimal,
        /// Target weight of the second resource
        target_weight2: Decimal,
        /// Current weight of the first resource
        weight1: Decimal,
        /// Current weight of the second resource
        weight2: Decimal,
        /// Duration of the bootstrap. Amount of days in which the target_weights are reached.
        duration: i64,
        /// Address of the first resource
        resource1: ResourceAddress,
        /// Address of the second resource
        resource2: ResourceAddress,
        /// Start time of the bootstrap
        start: Instant,
        /// Initial amount of the resource with the lowest initial weight
        initial_little_amount: Decimal,
        /// Address of the resource with the lowest initial weight
        initial_little_address: ResourceAddress,
        /// Vault holding the LP tokens
        lp_vault: Vault,
        /// Vault holding the reclaimable resource (will be filled with the initial_little_amount of the resource with the lowest initial weight)
        reclaimable_resource: Vault,
        /// Badge holding the bootstrap badge, used to reclaim the reclaimable resource after the bootstrap is finished
        bootstrap_badge_vault: Vault,
        /// dapp definition
        oci_dapp_definition: ComponentAddress,
    }

    impl LinearBootstrapPool {
        /// Instantiates a new LinearBootstrapPool component.
        /// 
        /// # Input
        /// - `resource1`: First resource of the pool
        /// - `resource2`: Second resource of the pool
        /// - `initial_weight1`: Initial weight of the first resource
        /// - `initial_weight2`: Initial weight of the second resource
        /// - `target_weight1`: Target weight of the first resource
        /// - `target_weight2`: Target weight of the second resource
        /// - `fee`: Fee to be paid on swaps
        /// - `duration`: Duration of the bootstrap. Amount of days in which the target_weights are reached.
        /// 
        /// # Output
        /// - `Global<LinearBootstrapPool>`: The newly instantiated LinearBootstrapPool component
        /// - `Option<Bucket>`: Empty bucket that can't be dropped (resource created by the pool component)
        /// - `Bucket`: Bucket containing the bootstrap badge
        /// 
        /// # Logic
        /// - Creating a bootstrap badge to reclaim resources after the bootstrap is finished
        /// - Instantiating a TwoResourcePool component with the given resources
        /// - Contributes the resources to the pool component
        /// - Stores resulting lp tokens in the lp_vault
        pub fn new(
            resource1: Bucket,
            resource2: Bucket,
            initial_weight1: Decimal,
            initial_weight2: Decimal,
            target_weight1: Decimal,
            target_weight2: Decimal,
            fee: Decimal,
            duration: i64,
            oci_dapp_definition: ComponentAddress,
        ) -> (Global<LinearBootstrapPool>, Option<Bucket>, Bucket) {
            let (address_reservation, component_address) =
                Runtime::allocate_component_address(LinearBootstrapPool::blueprint_id());
            let global_component_caller_badge =
                NonFungibleGlobalId::global_caller_badge(component_address);

            let mut bootstrap_badge: Bucket = ResourceBuilder::new_fungible(OwnerRole::None)
                .divisibility(DIVISIBILITY_MAXIMUM)
                .metadata(metadata! (
                    init {
                        "name" => "bootstrap badge", locked;
                        "symbol" => "BOOT", locked;
                    }
                ))
                .mint_roles(mint_roles!(
                    minter => rule!(require(global_caller(component_address)));
                    minter_updater => rule!(deny_all);
                ))
                .mint_initial_supply(2)
                .into();

            let mut pool_component = Blueprint::<TwoResourcePool>::instantiate(
                OwnerRole::Fixed(rule!(require(global_component_caller_badge.clone()))),
                rule!(
                    require(global_component_caller_badge)
                        || require_amount(dec!("2"), bootstrap_badge.resource_address())
                ),
                (resource1.resource_address(), resource2.resource_address()),
                None,
            );

            let resource1_address = resource1.resource_address();
            let resource2_address = resource2.resource_address();

            let (initial_little_amount, initial_little_address): (Decimal, ResourceAddress) =
                if initial_weight1 > initial_weight2 {
                    (resource2.amount(), resource2_address)
                } else {
                    (resource1.amount(), resource1_address)
                };

            let (lp_bucket, little_idiot_bucket): (Bucket, Option<Bucket>) = bootstrap_badge
                .authorize_with_all(|| pool_component.contribute((resource1, resource2)));

            let component = Self {
                pool_component,
                fee,
                initial_weight1,
                target_weight1,
                target_weight2,
                initial_weight2,
                weight1: initial_weight1,
                weight2: initial_weight2,
                duration,
                resource1: resource1_address,
                resource2: resource2_address,
                start: Clock::current_time_rounded_to_seconds(),
                initial_little_address,
                initial_little_amount,
                lp_vault: Vault::with_bucket(lp_bucket),
                reclaimable_resource: Vault::new(initial_little_address),
                bootstrap_badge_vault: Vault::with_bucket(bootstrap_badge.take(1)),
                oci_dapp_definition,
            }
            .instantiate()
            .prepare_to_globalize(OwnerRole::Fixed(rule!(require(
                bootstrap_badge.resource_address()
            ))))
            .with_address(address_reservation)
            .globalize();

            (component, little_idiot_bucket, bootstrap_badge)
        }

        /// Removes liquidity from the pool.
        /// 
        /// # Input
        /// - `pool_units`: Amount of LP tokens to redeem
        /// 
        /// # Output
        /// - `Bucket`: Bucket containing the first resource
        /// - `Bucket`: Bucket containing the second resource
        /// 
        /// # Logic
        /// - Updates the weights of the pool
        /// - Redeems the pool units from the pool component
        pub fn remove_liquidity(&mut self, pool_units: Bucket) -> (Bucket, Bucket) {
            self.set_weights();
            self.pool_component.redeem(pool_units)
        }

        /// Swaps one resource for another.
        /// 
        /// # Input
        /// - `input_bucket`: Bucket containing the input resource
        /// 
        /// # Output
        /// - `Bucket`: Bucket containing the output resource
        /// 
        /// # Logic
        /// - Updates the weights of the pool
        /// - Calculates the output amount based on the input amount and the reserves
        /// - Deposits the input resource in the pool
        /// - Withdraws the output resource from the pool
        /// - Returns the output resource
        pub fn swap(&mut self, input_bucket: Bucket) -> Bucket {
            self.set_weights();
            let mut reserves = self.vault_reserves();

            let input_reserves = reserves
                .remove(&input_bucket.resource_address())
                .expect("Resource does not belong to the pool");
            let (output_resource_address, output_reserves) = reserves.into_iter().next().unwrap();

            let input_amount = input_bucket.amount();

            // Get the weights based on the resource
            let (input_weight, output_weight) = if input_bucket.resource_address() == self.resource1
            {
                (self.weight1, self.weight2)
            } else {
                (self.weight2, self.weight1)
            };

            // Balancer-style swap formula considering weights
            let output_amount =
                (input_amount * output_reserves * output_weight * (dec!("1") - self.fee))
                    / (input_reserves * input_weight
                        + input_amount * output_weight * (dec!("1") - self.fee));

            self.deposit(input_bucket);

            self.withdraw(output_resource_address, output_amount)
        }

        /// Returns the price of the first resource in the pool.
        /// 
        /// # Input
        /// - None
        /// 
        /// # Output
        /// - `Decimal`: Price of the first resource
        /// 
        /// # Logic
        /// - Updates the weights of the pool
        /// - Calculates the price of the first resource based on the reserves and the weights
        pub fn get_resource1_price(&mut self) -> Decimal {
            self.set_weights();
            let reserves = self.vault_reserves();
            let resource1_reserve = *reserves.get(&self.resource1).unwrap();
            let resource2_reserve = *reserves.get(&self.resource2).unwrap();
            let weighted_price =
                (resource2_reserve * self.weight2) / (resource1_reserve * self.weight1);
            weighted_price
        }

        /// Finishes the bootstrap.
        /// 
        /// # Input
        /// - None
        /// 
        /// # Output
        /// - `Bucket`: Bucket containing the resulting lp tokens
        /// 
        /// # Logic
        /// - Redeems the LP tokens from the pool component
        /// - Checks which resource has the initial_little_amount and puts it in the reclaimable_resource vault
        /// - Instantiates a new BasicPool component with the leftover resources and the fee
        /// - Returns the resulting lp tokens
        pub fn finish_bootstrap(&mut self) -> Bucket {
            let progress = self.get_progress();
            assert!(progress >= dec!(1), "Bootstrap not finished yet");

            let (mut resource1, mut resource2): (Bucket, Bucket) =
                self.pool_component.redeem(self.lp_vault.take_all());

            if resource1.resource_address() == self.initial_little_address {
                self.reclaimable_resource
                    .put(resource1.take(self.initial_little_amount));
                let (_component, lp_tokens) = Blueprint::<BasicPool>::instantiate_with_liquidity(
                    resource1,
                    resource2,
                    self.fee,
                    self.oci_dapp_definition,
                );
                lp_tokens
            } else {
                self.reclaimable_resource
                    .put(resource2.take(self.initial_little_amount));
                let (_component, lp_tokens) = Blueprint::<BasicPool>::instantiate_with_liquidity(
                    resource1,
                    resource2,
                    self.fee,
                    self.oci_dapp_definition,
                );
                lp_tokens
            }
        }

        /// Reclaims the initial resources.
        /// 
        /// # Input
        /// - `boot_badge`: Bucket containing the bootstrap badge
        /// 
        /// # Output
        /// - `Bucket`: Bucket containing the initial resources
        /// 
        /// # Logic
        /// - Checks if the bootstrap badge is correct
        /// - Puts the bootstrap badge in the bootstrap_badge_vault
        /// - Takes all resources from the reclaimable_resource vault
        pub fn reclaim_initial(&mut self, boot_badge: Bucket) -> Bucket {
            assert!(boot_badge.resource_address() == self.bootstrap_badge_vault.resource_address());
            self.bootstrap_badge_vault.put(boot_badge);
            self.reclaimable_resource.take_all()
        }

        fn set_weights(&mut self) {
            let progress: Decimal = self.get_progress();

            if progress >= dec!(1) {
                self.weight1 = self.target_weight1;
                self.weight2 = self.target_weight2;
            } else {
                self.weight1 =
                    self.initial_weight1 + (self.target_weight1 - self.initial_weight1) * progress;
                self.weight2 =
                    self.initial_weight2 + (self.target_weight2 - self.initial_weight2) * progress;
            }
        }

        /// Returns the progress of the bootstrap.
        /// 
        /// # Input
        /// - None
        /// 
        /// # Output
        /// - `Decimal`: Progress of the bootstrap (0 to 1)
        /// 
        /// # Logic
        /// - Calculates the elapsed time since the start of the bootstrap
        /// - Calculates the time to elapse until the end of the bootstrap
        /// - Returns the progress as a decimal between 0 and 1
        fn get_progress(&self) -> Decimal {
            let elapsed_time = Clock::current_time_rounded_to_seconds().seconds_since_unix_epoch
                - self.start.seconds_since_unix_epoch;
            let time_to_elapse = self
                .start
                .add_days(self.duration)
                .unwrap()
                .seconds_since_unix_epoch
                - self.start.seconds_since_unix_epoch;
            Decimal::from(elapsed_time) / Decimal::from(time_to_elapse)
        }

        /// Returns the reserves of the pool.
        fn vault_reserves(&self) -> IndexMap<ResourceAddress, Decimal> {
            self.pool_component.get_vault_amounts()
        }

        /// Deposits a bucket in the pool.
        fn deposit(&mut self, bucket: Bucket) {
            self.pool_component.protected_deposit(bucket)
        }

        /// Withdraws a bucket from the pool.
        fn withdraw(&mut self, resource_address: ResourceAddress, amount: Decimal) -> Bucket {
            self.pool_component.protected_withdraw(
                resource_address,
                amount,
                WithdrawStrategy::Rounded(RoundingMode::ToZero),
            )
        }
    }
}
