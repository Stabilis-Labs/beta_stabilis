use scrypto::prelude::*;

#[blueprint]
mod stabilis_liquidity_pool {
    enable_method_auth! {
        methods {
            add_liquidity => PUBLIC;
            remove_liquidity => PUBLIC;
            get_stab_price => PUBLIC;
            swap => PUBLIC;
            set_fee => restrict_to: [OWNER];
        }
    }

    struct StabilisPool {
        pool_component: Global<TwoResourcePool>,
        fee: Decimal,
    }

    impl StabilisPool {
        pub fn new(
            owner_role: OwnerRole, //proxy owner badge
            resource_address1: ResourceAddress,
            resource_address2: ResourceAddress,
            fee: Decimal,
        ) -> Global<StabilisPool> {
            let (address_reservation, component_address) =
                Runtime::allocate_component_address(StabilisPool::blueprint_id());
            let global_component_caller_badge =
                NonFungibleGlobalId::global_caller_badge(component_address);

            let pool_component = Blueprint::<TwoResourcePool>::instantiate(
                owner_role.clone(),
                rule!(require(global_component_caller_badge)),
                (resource_address1, resource_address2),
                None,
            );

            Self {
                pool_component,
                fee,
            }
            .instantiate()
            .prepare_to_globalize(owner_role)
            .with_address(address_reservation)
            .globalize()
        }

        pub fn add_liquidity(
            &mut self,
            resource1: Bucket,
            resource2: Bucket,
        ) -> (Bucket, Option<Bucket>) {
            self.pool_component.contribute((resource1, resource2))
        }

        pub fn remove_liquidity(&mut self, pool_units: Bucket) -> (Bucket, Bucket) {
            self.pool_component.redeem(pool_units)
        }

        pub fn swap(&mut self, input_bucket: Bucket) -> Bucket {
            let mut reserves = self.vault_reserves();

            let input_reserves = reserves
                .remove(&input_bucket.resource_address())
                .expect("Resource does not belong to the pool");
            let (output_resource_address, output_reserves) = reserves.into_iter().next().unwrap();

            let input_amount = input_bucket.amount();

            let output_amount = (input_amount * output_reserves * (dec!("1") - self.fee))
                / (input_reserves + input_amount * (dec!("1") - self.fee));

            self.deposit(input_bucket);

            self.withdraw(output_resource_address, output_amount)
        }

        pub fn get_stab_price(&self) -> Decimal {
            let reserves = self.vault_reserves();
            let first_amount: Decimal = *reserves.first().map(|(_, v)| v).unwrap();
            let last_amount: Decimal = *reserves.last().map(|(_, v)| v).unwrap();
            last_amount / first_amount
        }

        pub fn set_fee(&mut self, fee: Decimal) {
            self.fee = fee;
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
