use crate::flash_loans::flash_loans::*;
use crate::stabilis_liquidity_pool::stabilis_liquidity_pool::*;
use crate::structs::*;
use scrypto::prelude::*;
use scrypto_math::*;

#[blueprint]
mod proxy {
    enable_method_auth! {
        methods {
            open_cdp => PUBLIC;
            top_up_cdp => PUBLIC;
            remove_collateral => PUBLIC;
            close_cdp => PUBLIC;
            retrieve_leftover_collateral => PUBLIC;
            save_cdp => PUBLIC;
            mark_for_liquidation => PUBLIC;
            liquidate_position_with_marker => PUBLIC;
            liquidate_position_without_marker => PUBLIC;
            update => PUBLIC;
            get_internal_price => PUBLIC;
            flash_borrow => PUBLIC;
            flash_pay_back => PUBLIC;
            burn_marker => PUBLIC;
            burn_loan_receipt => PUBLIC;
            force_mint => PUBLIC;
            force_liquidate => PUBLIC;
            receive_badges => PUBLIC;
            change_collateral_price => restrict_to: [OWNER];
            set_max_vector_length => restrict_to: [OWNER];
            set_price_error => restrict_to: [OWNER];
            set_allowed_deviation => restrict_to: [OWNER];
            set_minmax_interest => restrict_to: [OWNER];
            set_update_delays => restrict_to: [OWNER];
            set_ks => restrict_to: [OWNER];
            add_collateral => restrict_to: [OWNER];
            add_pool_collateral => restrict_to: [OWNER];
            change_internal_price => restrict_to: [OWNER];
            set_oracle => restrict_to: [OWNER];
            send_badges => restrict_to: [OWNER];
            burn_badges => restrict_to: [OWNER];
            flash_retrieve_interest => restrict_to: [OWNER];
            set_force_mint_liq_percentage => restrict_to: [OWNER];
            set_number_of_prices_cached => restrict_to: [OWNER];
        }
    }

    extern_blueprint! {
        "package_tdx_2_1pkqn52t324ezshectwlwkzk0w8zqamyq6aqugkmq9q7zcvhshfq0s2",
        Stabilis {
            fn open_cdp(&mut self, collateral: Bucket, stab_to_mint: Decimal, safe: bool) -> (Bucket, Bucket);
            fn close_cdp(&mut self, receipt_proof: NonFungibleLocalId, stab_payment: Bucket) -> (Bucket, Bucket);
            fn retrieve_leftover_collateral(&mut self, receipt_proof: NonFungibleLocalId) -> Bucket;
            fn top_up_cdp(&mut self, receipt_proof: NonFungibleLocalId, collateral: Bucket) -> ();
            fn save_cdp(&mut self, receipt_proof: NonFungibleLocalId, collateral: Bucket) -> ();
            fn mark_for_liquidation(&mut self, collateral: ResourceAddress) -> NonFungibleBucket;
            fn liquidate_position_with_marker(&mut self, marker_receipt: NonFungibleLocalId, payment: Bucket) -> (Bucket, Option<Bucket>, Bucket);
            fn liquidate_position_without_marker(&mut self, payment: Bucket, automatic: bool, skip: i64, cdp_id: NonFungibleLocalId) -> (Bucket, Option<Bucket>, Bucket);
            fn change_collateral_price(&self, collateral: ResourceAddress, new_price: Decimal) -> ();
            fn add_collateral(&self, address: ResourceAddress, chosen_mcr: Decimal, initial_price: Decimal) -> ();
            fn add_pool_collateral(&self, address: ResourceAddress, parent_address: ResourceAddress, validator: ComponentAddress, lsu: bool, initial_acceptance: bool) -> ();
            fn change_internal_price(&mut self, new_price: Decimal) -> ();
            fn empty_collateral_treasury(&mut self, amount: Decimal, collateral: ResourceAddress, error_fallback: bool) -> Bucket;
            fn mint_controller_badge(&self, amount: Decimal) -> Bucket;
            fn edit_collateral(&mut self, address: ResourceAddress, new_mcr: Decimal, new_acceptance: bool, new_max_share: Decimal) -> ();
            fn edit_pool_collateral(&mut self, address: ResourceAddress, new_acceptance: bool, new_max_share: Decimal) -> ();
            fn set_liquidation_delay(&mut self, new_delay: i64) -> ();
            fn set_unmarked_delay(&mut self, new_delay: i64) -> ();
            fn set_max_vector_length(&mut self, new_max_length: u64) -> ();
            fn set_minimum_mint(&mut self, new_minimum_mint: Decimal) -> ();
            fn set_fines(&mut self, liquidator_fine: Decimal, pool_liquidator_fine: Decimal, stabilis_fine: Decimal) -> ();
            fn return_internal_price(&self) -> Decimal;
            fn remove_collateral(&mut self, receipt_proof: NonFungibleLocalId, amount: Decimal) -> Bucket;
            fn get_marker_manager(&self) -> ResourceManager;
            fn force_liquidate(&mut self, collateral: ResourceAddress, payment: Bucket, percentage_to_take: Decimal) -> (Bucket, Bucket);
            fn force_mint(&mut self, collateral: ResourceAddress, payment: Bucket, percentage_to_supply: Decimal) -> (Bucket, Option<Bucket>);
            fn burn_marker(&self, marker: Bucket);
            fn burn_loan_receipt(&self, receipt: Bucket);
        }
    }

    const STABILIS: Global<Stabilis> = global_component!(
        Stabilis,
        "component_tdx_2_1cp4j27fcr4e74g59euhje8wk4u4q0jq358jf8u4j4z7znfqnj6jx0q"
    );

    struct Proxy {
        badge_vault: FungibleVault,
        stab_pool: Global<StabilisPool>,
        stabilis: Global<Stabilis>,
        oracle: Global<AnyComponent>,
        oracle_method_name: String,
        flash_loans: Global<FlashLoans>,
        latest_stab_price_errors: KeyValueStore<u64, Decimal>,
        latest_stab_price_errors_total: Decimal,
        last_update: Instant,
        update_delay: i64,
        number_of_cached_prices: u64,
        last_changed_price: u64,
        internal_price: Decimal,
        full_cache: bool,
        interest_rate: Decimal,
        kp: Decimal,
        ki: Decimal,
        cdp_receipt_manager: ResourceManager,
        cdp_marker_manager: ResourceManager,
        xrd_price: Decimal,
        max_vector_length: usize,
        max_interest_rate: Decimal,
        min_interest_rate: Decimal,
        allowed_deviation: Decimal,
        max_price_error: Decimal,
        price_error_offset: Decimal,
        accepted_collaterals: Vec<ResourceAddress>,
        percentage_to_supply: Decimal,
        percentage_to_take: Decimal,
    }

    impl Proxy {
        pub fn new(
            xrd_bucket: Bucket,
            stab_bucket: Bucket,
            mut controller_badge: Bucket,
            cdp_receipt_address: ResourceAddress,
            cdp_marker_address: ResourceAddress,
            oracle_address: ComponentAddress,
        ) -> (Global<Proxy>, Bucket, Bucket, Option<Bucket>) {
            let (address_reservation, component_address) =
                Runtime::allocate_component_address(Proxy::blueprint_id());

            let proxy_owner: FungibleBucket = ResourceBuilder::new_fungible(OwnerRole::None)
                .divisibility(DIVISIBILITY_MAXIMUM)
                .metadata(metadata! (
                    init {
                        "name" => "owner badge proxy", locked;
                        "symbol" => "stabPROX", locked;
                    }
                ))
                .mint_roles(mint_roles!(
                    minter => rule!(require(global_caller(component_address)));
                    minter_updater => rule!(deny_all);
                ))
                .mint_initial_supply(1);

            let stab_pool: Global<StabilisPool> = StabilisPool::new(
                OwnerRole::Fixed(rule!(require(proxy_owner.resource_address()))),
                stab_bucket.resource_address(),
                xrd_bucket.resource_address(),
                dec!(0.001),
            );

            let (lp_tokens, optional_return_bucket): (Bucket, Option<Bucket>) =
                stab_pool.add_liquidity(stab_bucket, xrd_bucket);

            let internal_price: Decimal =
                controller_badge.authorize_with_all(|| STABILIS.return_internal_price());

            let proxy = Self {
                flash_loans: FlashLoans::instantiate(controller_badge.take(1)),
                badge_vault: FungibleVault::with_bucket(controller_badge.as_fungible()),
                stab_pool,
                stabilis: STABILIS,
                oracle: Global::from(oracle_address),
                oracle_method_name: "get_prices".to_string(),
                last_update: Clock::current_time_rounded_to_minutes(),
                update_delay: 0,
                latest_stab_price_errors: KeyValueStore::new(),
                latest_stab_price_errors_total: dec!(0),
                number_of_cached_prices: 50,
                last_changed_price: 0,
                full_cache: false,
                internal_price,
                interest_rate: dec!(1),
                kp: dec!("0.00000000076517857"),
                ki: dec!("0.00000000076517857"),
                cdp_receipt_manager: ResourceManager::from_address(cdp_receipt_address),
                cdp_marker_manager: ResourceManager::from_address(cdp_marker_address),
                xrd_price: dec!("0.041"),
                max_vector_length: 100usize,
                max_interest_rate: dec!("1.0000007715"),
                min_interest_rate: dec!("0.9999992287"),
                allowed_deviation: dec!("0.005"),
                max_price_error: dec!("0.5"),
                price_error_offset: dec!(1),
                accepted_collaterals: vec![XRD],
                percentage_to_supply: dec!("1.05"),
                percentage_to_take: dec!("0.95"),
            }
            .instantiate()
            .prepare_to_globalize(OwnerRole::Fixed(rule!(require(
                proxy_owner.resource_address()
            ))))
            .with_address(address_reservation)
            .globalize();

            (proxy, proxy_owner.into(), lp_tokens, optional_return_bucket)
        }

        pub fn update(&mut self) {
            let passed_minutes: Decimal = (Clock::current_time_rounded_to_minutes()
                .seconds_since_unix_epoch
                - self.last_update.seconds_since_unix_epoch)
                / dec!(60);

            self.update_collateral_prices();

            if passed_minutes >= Decimal::from(self.update_delay) {
                self.update_internal_price();
            }
        }

        ////////////////////////////////////////////////////////////////////
        //////////////////////////ADMIN METHODS/////////////////////////////
        ////////////////////////////////////////////////////////////////////

        pub fn set_price_error(&mut self, new_max: Decimal, new_offset: Decimal) {
            self.max_price_error = new_max;
            self.price_error_offset = new_offset;
        }

        pub fn set_allowed_deviation(&mut self, allowed_deviation: Decimal) {
            self.allowed_deviation = allowed_deviation;
        }

        pub fn set_number_of_prices_cached(&mut self, new_number: u64) {
            self.number_of_cached_prices = new_number;
            self.latest_stab_price_errors_total = dec!(0);
            self.last_changed_price = 0;
            self.full_cache = false;
        }

        pub fn set_force_mint_liq_percentage(
            &mut self,
            percentage_to_supply: Decimal,
            percentage_to_take: Decimal,
        ) {
            self.percentage_to_supply = percentage_to_supply;
            self.percentage_to_take = percentage_to_take;
        }

        pub fn set_minmax_interest(&mut self, min_interest: Decimal, max_interest: Decimal) {
            self.max_interest_rate = max_interest;
            self.min_interest_rate = min_interest;
        }

        pub fn set_update_delays(&mut self, update_delay: i64) {
            self.update_delay = update_delay;
        }

        pub fn set_ks(&mut self, new_ki: Decimal, new_kp: Decimal) {
            self.ki = new_ki;
            self.kp = new_kp;
        }

        pub fn set_oracle(&mut self, oracle_address: ComponentAddress, method_name: String) {
            self.oracle = Global::from(oracle_address);
            self.oracle_method_name = method_name;
        }

        ////////////////////////////////////////////////////////////////////
        //////////////////////////HELPER METHODS////////////////////////////
        ////////////////////////////////////////////////////////////////////

        fn update_collateral_prices(&mut self) {
            let prices: Vec<(ResourceAddress, Decimal)> =
                self.oracle.call(&self.oracle_method_name, &());
            for (address, price) in prices {
                if address == XRD {
                    self.xrd_price = price;
                }
                if self.accepted_collaterals.contains(&address) {
                    self.badge_vault.authorize_with_amount(dec!("0.75"), || {
                        self.stabilis.change_collateral_price(address, price)
                    });
                }
            }
        }

        fn update_internal_price(&mut self) {
            let mut price_error: Decimal =
                self.stab_pool.get_stab_price() * self.xrd_price * self.price_error_offset
                    - self.internal_price;

            if price_error > self.max_price_error {
                price_error = self.max_price_error;
            }

            let passed_minutes: Decimal = (Clock::current_time_rounded_to_minutes()
                .seconds_since_unix_epoch
                - self.last_update.seconds_since_unix_epoch)
                / dec!(60);

            let to_change_id: u64 = match self.last_changed_price >= self.number_of_cached_prices {
                true => {
                    self.full_cache = true;
                    1
                }
                false => self.last_changed_price + 1,
            };

            if !self.full_cache {
                self.latest_stab_price_errors_total += price_error;
            } else {
                self.latest_stab_price_errors_total +=
                    price_error - *self.latest_stab_price_errors.get(&to_change_id).unwrap();
            }

            self.last_changed_price = to_change_id;
            self.latest_stab_price_errors
                .insert(to_change_id, price_error);

            if price_error.checked_abs().unwrap() > self.allowed_deviation * self.internal_price {
                self.interest_rate -= (self.kp * (price_error / self.internal_price)
                    + self.ki
                        * (self.latest_stab_price_errors_total
                            / (self.internal_price * Decimal::from(self.number_of_cached_prices))))
                    * passed_minutes;

                if self.interest_rate > self.max_interest_rate {
                    self.interest_rate = self.max_interest_rate;
                } else if self.interest_rate < self.min_interest_rate {
                    self.interest_rate = self.min_interest_rate;
                }
            }

            let calculated_price: Decimal =
                self.internal_price * self.interest_rate.pow(passed_minutes).unwrap();

            self.last_update = Clock::current_time_rounded_to_minutes();
            self.change_internal_price(calculated_price);
        }

        ////////////////////////////////////////////////////////////////////
        /////////////////////////MIGRATION METHODS//////////////////////////
        ////////////////////////////////////////////////////////////////////

        pub fn receive_badges(&mut self, badge_bucket: Bucket) {
            self.badge_vault.put(badge_bucket.as_fungible());
        }

        pub fn send_badges(&mut self, amount: Decimal, receiver_address: ComponentAddress) {
            let receiver: Global<AnyComponent> = Global::from(receiver_address);
            let badge_bucket: Bucket = self.badge_vault.take(amount).into();
            receiver.call_raw("receive_badges", scrypto_args!(badge_bucket))
        }

        pub fn burn_badges(&mut self, amount: Decimal, all: bool) {
            if all {
                self.badge_vault.take_all().burn();
            } else {
                self.badge_vault.take(amount).burn();
            }
        }

        ////////////////////////////////////////////////////////////////////
        ///////////////////CONTROLLING OTHER COMPONENTS/////////////////////
        ////////////////////////////////////////////////////////////////////

        ////////////////////////////////////////////////////////////////////
        ////////////////////////STABILIS COMPONENT//////////////////////////
        ////////////////////////////////////////////////////////////////////

        pub fn open_cdp(
            &mut self,
            collateral: Bucket,
            stab_to_mint: Decimal,
            safe: bool,
        ) -> (Bucket, Bucket) {
            self.badge_vault.authorize_with_amount(dec!("0.75"), || {
                self.stabilis.open_cdp(collateral, stab_to_mint, safe)
            })
        }

        pub fn add_collateral(
            &mut self,
            address: ResourceAddress,
            chosen_mcr: Decimal,
            initial_price: Decimal,
        ) {
            self.badge_vault.authorize_with_amount(dec!("0.75"), || {
                self.stabilis
                    .add_collateral(address, chosen_mcr, initial_price)
            });
            self.accepted_collaterals.push(address);
        }

        pub fn remove_collateral(
            &mut self,
            receipt_proof: NonFungibleProof,
            amount: Decimal,
        ) -> Bucket {
            let receipt_proof = receipt_proof.check_with_message(
                self.cdp_receipt_manager.address(),
                "Incorrect proof! Are you sure this loan is yours?",
            );
            let receipt = receipt_proof.non_fungible::<Cdp>();
            let receipt_id: NonFungibleLocalId = receipt.local_id().clone();

            self.badge_vault.authorize_with_amount(dec!("0.75"), || {
                self.stabilis.remove_collateral(receipt_id, amount)
            })
        }

        pub fn close_cdp(
            &mut self,
            receipt_proof: NonFungibleProof,
            stab_payment: Bucket,
        ) -> (Bucket, Bucket) {
            let receipt_proof = receipt_proof.check_with_message(
                self.cdp_receipt_manager.address(),
                "Incorrect proof! Are you sure this loan is yours?",
            );
            let receipt = receipt_proof.non_fungible::<Cdp>();
            let receipt_id: NonFungibleLocalId = receipt.local_id().clone();

            self.badge_vault.authorize_with_amount(dec!("0.75"), || {
                self.stabilis.close_cdp(receipt_id, stab_payment)
            })
        }

        pub fn retrieve_leftover_collateral(&mut self, receipt_proof: NonFungibleProof) -> Bucket {
            let receipt_proof = receipt_proof.check_with_message(
                self.cdp_receipt_manager.address(),
                "Incorrect proof! Are you sure this loan is yours?",
            );
            let receipt = receipt_proof.non_fungible::<Cdp>();
            let receipt_id: NonFungibleLocalId = receipt.local_id().clone();

            self.badge_vault.authorize_with_amount(dec!("0.75"), || {
                self.stabilis.retrieve_leftover_collateral(receipt_id)
            })
        }

        pub fn top_up_cdp(&mut self, receipt_proof: NonFungibleProof, collateral: Bucket) {
            let receipt_proof = receipt_proof.check_with_message(
                self.cdp_receipt_manager.address(),
                "Incorrect proof! Are you sure this loan is yours?",
            );
            let receipt = receipt_proof.non_fungible::<Cdp>();
            let receipt_id: NonFungibleLocalId = receipt.local_id().clone();

            self.badge_vault.authorize_with_amount(dec!("0.75"), || {
                self.stabilis.top_up_cdp(receipt_id, collateral)
            });
        }

        pub fn save_cdp(&mut self, receipt_proof: NonFungibleProof, collateral: Bucket) {
            let receipt_proof = receipt_proof.check_with_message(
                self.cdp_receipt_manager.address(),
                "Incorrect proof! Are you sure this loan is yours?",
            );
            let receipt = receipt_proof.non_fungible::<Cdp>();
            let receipt_id: NonFungibleLocalId = receipt.local_id().clone();

            self.badge_vault.authorize_with_amount(dec!("0.75"), || {
                self.stabilis.save_cdp(receipt_id, collateral)
            });
        }

        pub fn mark_for_liquidation(&mut self, collateral: ResourceAddress) -> NonFungibleBucket {
            self.badge_vault.authorize_with_amount(dec!("0.75"), || {
                self.stabilis.mark_for_liquidation(collateral)
            })
        }

        pub fn burn_marker(&self, marker: Bucket) {
            self.badge_vault
                .authorize_with_amount(dec!("0.75"), || self.stabilis.burn_marker(marker));
        }

        pub fn burn_loan_receipt(&self, receipt: Bucket) {
            self.badge_vault
                .authorize_with_amount(dec!("0.75"), || self.stabilis.burn_loan_receipt(receipt));
        }

        pub fn liquidate_position_with_marker(
            &mut self,
            marker_proof: NonFungibleProof,
            payment: Bucket,
        ) -> (Bucket, Option<Bucket>, Bucket) {
            let marker_proof = marker_proof.check_with_message(
                self.cdp_marker_manager.address(),
                "Incorrect proof! Are you sure this is a correct marker?",
            );
            let marker = marker_proof.non_fungible::<CdpMarker>();
            let marker_id: NonFungibleLocalId = marker.local_id().clone();

            self.badge_vault.authorize_with_amount(dec!("0.75"), || {
                self.stabilis
                    .liquidate_position_with_marker(marker_id, payment)
            })
        }

        pub fn force_liquidate(
            &mut self,
            collateral: ResourceAddress,
            payment: Bucket,
        ) -> (Bucket, Bucket) {
            self.badge_vault.authorize_with_amount(dec!("0.75"), || {
                self.stabilis
                    .force_liquidate(collateral, payment, self.percentage_to_take)
            })
        }

        pub fn force_mint(
            &mut self,
            collateral: ResourceAddress,
            payment: Bucket,
        ) -> (Bucket, Option<Bucket>) {
            self.badge_vault.authorize_with_amount(dec!("0.75"), || {
                self.stabilis
                    .force_mint(collateral, payment, self.percentage_to_supply)
            })
        }

        pub fn liquidate_position_without_marker(
            &mut self,
            payment: Bucket,
            automatic: bool,
            skip: i64,
            cdp_id: NonFungibleLocalId,
        ) -> (Bucket, Option<Bucket>, Bucket) {
            self.badge_vault.authorize_with_amount(dec!("0.75"), || {
                self.stabilis
                    .liquidate_position_without_marker(payment, automatic, skip, cdp_id)
            })
        }

        pub fn change_collateral_price(&self, collateral: ResourceAddress, new_price: Decimal) {
            self.badge_vault.authorize_with_amount(dec!("0.75"), || {
                self.stabilis.change_collateral_price(collateral, new_price)
            });
        }

        pub fn add_pool_collateral(
            &self,
            address: ResourceAddress,
            parent_address: ResourceAddress,
            validator: ComponentAddress,
            lsu: bool,
            initial_acceptance: bool,
        ) {
            self.badge_vault.authorize_with_amount(dec!("0.75"), || {
                self.stabilis.add_pool_collateral(
                    address,
                    parent_address,
                    validator,
                    lsu,
                    initial_acceptance,
                )
            });
        }

        pub fn change_internal_price(&mut self, new_price: Decimal) {
            self.internal_price = new_price;
            self.badge_vault.authorize_with_amount(dec!("0.75"), || {
                self.stabilis.change_internal_price(new_price)
            });
        }

        pub fn set_max_vector_length(&mut self, new_stabilis_length: u64, new_own_length: u64) {
            self.badge_vault.authorize_with_amount(dec!("0.75"), || {
                self.stabilis.set_max_vector_length(new_stabilis_length)
            });
            self.max_vector_length = new_own_length.to_usize().unwrap();
        }

        pub fn get_internal_price(&self) -> Decimal {
            self.internal_price
        }

        ////////////////////////////////////////////////////////////////////
        ///////////////////// FLASH LOAN COMPONENT /////////////////////////
        ////////////////////////////////////////////////////////////////////

        pub fn flash_borrow(&mut self, amount: Decimal) -> (Bucket, Bucket) {
            self.badge_vault
                .authorize_with_amount(dec!("0.75"), || self.flash_loans.borrow(amount))
        }

        pub fn flash_pay_back(&mut self, receipt_bucket: Bucket, payment_bucket: Bucket) -> Bucket {
            self.badge_vault.authorize_with_amount(dec!("0.75"), || {
                self.flash_loans.pay_back(receipt_bucket, payment_bucket)
            })
        }

        pub fn flash_retrieve_interest(&mut self) -> Bucket {
            self.badge_vault
                .authorize_with_amount(dec!("0.75"), || self.flash_loans.retrieve_interest())
        }
    }
}
