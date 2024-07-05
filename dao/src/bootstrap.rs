use scrypto::prelude::*;

#[blueprint]
mod bootstrap {
    enable_method_auth! {
        methods {
            remove_liquidity => PUBLIC;
            get_resource1_price => PUBLIC;
            swap => PUBLIC;
            finish_bootstrap => PUBLIC;
            reclaim_initial => restrict_to: [OWNER];
        }
    }
    struct LinearBootstrapPool {
        pool_component: Global<TwoResourcePool>,
        fee: Decimal,
        initial_weight1: Decimal,
        initial_weight2: Decimal,
        target_weight1: Decimal,
        target_weight2: Decimal,
        weight1: Decimal,
        weight2: Decimal,
        duration: i64,
        resource1: ResourceAddress,
        resource2: ResourceAddress,
        start: Instant,
        initial_little_amount: Decimal,
        initial_little_address: ResourceAddress,
        lp_vault: Vault,
        reclaimable_resource: Vault,
    }

    impl LinearBootstrapPool {
        pub fn new(
            resource1: Bucket,
            resource2: Bucket,
            initial_weight1: Decimal,
            initial_weight2: Decimal,
            target_weight1: Decimal,
            target_weight2: Decimal,
            fee: Decimal,
            duration: i64,
        ) -> (Global<LinearBootstrapPool>, Option<Bucket>, Bucket) {
            let (address_reservation, component_address) =
                Runtime::allocate_component_address(LinearBootstrapPool::blueprint_id());
            let global_component_caller_badge =
                NonFungibleGlobalId::global_caller_badge(component_address);

            let mut pool_component = Blueprint::<TwoResourcePool>::instantiate(
                OwnerRole::Fixed(rule!(require(global_component_caller_badge.clone()))),
                rule!(require(global_component_caller_badge)),
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

            let (lp_bucket, little_idiot_bucket): (Bucket, Option<Bucket>) =
                pool_component.contribute((resource1, resource2));
            let rm: ResourceManager = ResourceManager::from(lp_bucket.resource_address());
            rm.set_metadata("symbol".to_owned(), "LPBOOT".to_owned());
            rm.set_metadata(
                "name".to_owned(),
                "Liquidity Bootstrap Pool Token".to_owned(),
            );

            let bootstrap_badge: Bucket = ResourceBuilder::new_fungible(OwnerRole::None)
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
                .mint_initial_supply(1)
                .into();

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
            }
            .instantiate()
            .prepare_to_globalize(OwnerRole::Fixed(rule!(require(
                bootstrap_badge.resource_address()
            ))))
            .with_address(address_reservation)
            .globalize();

            (component, little_idiot_bucket, bootstrap_badge)
        }

        pub fn remove_liquidity(&mut self, pool_units: Bucket) -> (Bucket, Bucket) {
            self.set_weights();
            self.pool_component.redeem(pool_units)
        }

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

        pub fn get_resource1_price(&mut self) -> Decimal {
            self.set_weights();
            let reserves = self.vault_reserves();
            let resource1_reserve = *reserves.get(&self.resource1).unwrap();
            let resource2_reserve = *reserves.get(&self.resource2).unwrap();
            let weighted_price =
                (resource2_reserve * self.weight2) / (resource1_reserve * self.weight1);
            weighted_price
        }

        pub fn finish_bootstrap(&mut self) -> Bucket {
            let (resource1, resource2): (Bucket, Bucket) =
                self.pool_component.redeem(self.lp_vault.take_all());

            if resource1.resource_address() == self.initial_little_address {
                self.reclaimable_resource
                    .put(resource1 /*.take(self.initial_little_amount)*/);
                //SEND MY BOYS TO OCI HERE
                resource2
            } else {
                self.reclaimable_resource
                    .put(resource2 /*.take(self.initial_little_amount)*/);
                //SEND MY BOYS TO OCI HERE
                resource1
            }
        }

        pub fn reclaim_initial(&mut self) -> Bucket {
            self.reclaimable_resource.take_all()
        }

        fn set_weights(&mut self) {
            let elapsed_time = Clock::current_time_rounded_to_seconds().seconds_since_unix_epoch
                - self.start.seconds_since_unix_epoch;
            let time_to_elapse = self
                .start
                .add_days(self.duration)
                .unwrap()
                .seconds_since_unix_epoch
                - self.start.seconds_since_unix_epoch;
            let progress = Decimal::from(elapsed_time) / Decimal::from(time_to_elapse);

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

        fn vault_reserves(&self) -> IndexMap<ResourceAddress, Decimal> {
            self.pool_component.get_vault_amounts()
        }

        fn deposit(&mut self, bucket: Bucket) {
            self.pool_component.protected_deposit(bucket)
        }

        fn withdraw(&mut self, resource_address: ResourceAddress, amount: Decimal) -> Bucket {
            self.pool_component.protected_withdraw(
                resource_address,
                amount,
                WithdrawStrategy::Rounded(RoundingMode::ToZero),
            )
        }
    }
}
