use crate::nft_structs::*;
use scrypto::prelude::*;
use scrypto_avltree::AvlTree;

#[blueprint]
mod stabilis {
    enable_method_auth! {
        methods {
            return_internal_price => PUBLIC;
            add_pool_collateral => restrict_to: [OWNER];
            open_cdp => restrict_to: [OWNER];
            top_up_cdp => restrict_to: [OWNER];
            close_cdp => restrict_to: [OWNER];
            retrieve_leftover_collateral => restrict_to: [OWNER];
            mark_for_liquidation => restrict_to: [OWNER];
            liquidate_position_with_marker => restrict_to: [OWNER];
            liquidate_position_without_marker => restrict_to: [OWNER];
            change_collateral_price => restrict_to: [OWNER];
            empty_collateral_treasury => restrict_to: [OWNER];
            edit_collateral => restrict_to: [OWNER];
            edit_pool_collateral => restrict_to: [OWNER];
            mint_controller_badge => restrict_to: [OWNER];
            set_liquidation_delay => restrict_to: [OWNER];
            set_unmarked_delay => restrict_to: [OWNER];
            set_stops => restrict_to: [OWNER];
            set_max_vector_length => restrict_to: [OWNER];
            set_minimum_mint => restrict_to: [OWNER];
            set_fines => restrict_to: [OWNER];
            add_collateral => restrict_to: [OWNER];
            change_internal_price => restrict_to: [OWNER];
            remove_collateral => restrict_to: [OWNER];
            force_liquidate => restrict_to: [OWNER];
            force_mint => restrict_to: [OWNER];
            set_force_mint_multiplier => restrict_to: [OWNER];
            free_stab => restrict_to: [OWNER];
            burn_stab => restrict_to: [OWNER];
            burn_marker => restrict_to: [OWNER];
            burn_loan_receipt => restrict_to: [OWNER];
            borrow_more => restrict_to: [OWNER];
            partial_close_cdp => restrict_to: [OWNER];
        }
    }
    struct Stabilis {
        collaterals: KeyValueStore<ResourceAddress, CollateralInfo>,
        pool_units: KeyValueStore<ResourceAddress, PoolUnitInfo>,
        collateral_ratios:
            KeyValueStore<ResourceAddress, AvlTree<Decimal, Vec<NonFungibleLocalId>>>,
        cdp_counter: u64,
        cdp_manager: ResourceManager,
        stab_manager: ResourceManager,
        controller_badge_manager: ResourceManager,
        internal_stab_price: Decimal,
        circulating_stab: Decimal,
        cdp_marker_manager: ResourceManager,
        cdp_marker_counter: u64,
        marked_cdps: AvlTree<Decimal, NonFungibleLocalId>,
        marked_cdps_active: u64,
        marker_placing_counter: Decimal,
        liquidation_receipt_manager: ResourceManager,
        liquidation_counter: u64,
        parameters: ProtocolParameters,
    }

    impl Stabilis {
        pub fn instantiate() -> (Global<Stabilis>, Bucket) {
            //set protocol parameters
            let parameters = ProtocolParameters {
                minimum_mint: dec!(1),
                max_vector_length: 250,
                liquidation_delay: 5,
                unmarked_delay: 5,
                liquidation_liquidation_fine: dec!("0.10"),
                stabilis_liquidation_fine: dec!("0.05"),
                stop_liquidations: false,
                stop_openings: false,
                stop_closings: false,
                stop_force_mint: false,
                stop_force_liquidate: false,
                force_mint_cr_multiplier: dec!(3),
            };
            //assign component_address
            let (address_reservation, component_address) =
                Runtime::allocate_component_address(Stabilis::blueprint_id());

            //create the controller badge
            let controller_role: Bucket = ResourceBuilder::new_fungible(OwnerRole::Fixed(rule!(
                require(global_caller(component_address))
            )))
            .divisibility(DIVISIBILITY_MAXIMUM)
            .metadata(metadata! (
                init {
                    "name" => "controller badge stabilis", locked;
                    "symbol" => "stabCTRL", locked;
                }
            ))
            .mint_roles(mint_roles!(
                minter => rule!(require(global_caller(component_address)));
                minter_updater => rule!(deny_all);
            ))
            .mint_initial_supply(10)
            .into();

            let controller_badge_manager: ResourceManager = controller_role.resource_manager();

            //create the usds manager
            let stab_manager: ResourceManager = ResourceBuilder::new_fungible(OwnerRole::Fixed(
                rule!(require(controller_role.resource_address())),
            ))
            .divisibility(DIVISIBILITY_MAXIMUM)
            .metadata(metadata! (
                init {
                    "name" => "STAB token", updatable;
                    "symbol" => "STAB", updatable;
                    "info_url" => "https://stabilis.finance", updatable;
                    "icon_url" => Url::of("https://imgur.com/fEwyP5f.png"), updatable;
                }
            ))
            .mint_roles(mint_roles!(
                minter => rule!(require(global_caller(component_address))
                || require_amount(
                    dec!("0.75"),
                    controller_role.resource_address()
                ));
                minter_updater => rule!(require_amount(
                    dec!("0.75"),
                    controller_role.resource_address()
                ));
            ))
            .burn_roles(burn_roles!(
                burner => rule!(require(global_caller(component_address))
                || require_amount(
                    dec!("0.75"),
                    controller_role.resource_address()
                ));
                burner_updater => rule!(require_amount(
                    dec!("0.75"),
                    controller_role.resource_address()
                ));
            ))
            .create_with_no_initial_supply();

            //create the cdp manager
            let cdp_manager: ResourceManager =
                ResourceBuilder::new_integer_non_fungible::<Cdp>(OwnerRole::Fixed(rule!(
                    require_amount(dec!("0.75"), controller_role.resource_address())
                )))
                .metadata(metadata!(
                    init {
                        "name" => "Stabilis Loan Receipt", locked;
                        "symbol" => "stabLOAN", locked;
                        "description" => "A receipt for your Stabilis loan", locked;
                        "info_url" => "https://stabilis.finance", updatable;
                        "icon_url" => Url::of("https://i.imgur.com/pUFclTo.png"), updatable;
                    }
                ))
                .non_fungible_data_update_roles(non_fungible_data_update_roles!(
                    non_fungible_data_updater => rule!(require(global_caller(component_address))
                        || require_amount(
                            dec!("0.75"),
                            controller_role.resource_address()
                        ));
                    non_fungible_data_updater_updater => rule!(require_amount(
                        dec!("0.75"),
                        controller_role.resource_address()
                    ));
                ))
                .mint_roles(mint_roles!(
                    minter => rule!(require(global_caller(component_address))
                    || require_amount(
                        dec!("0.75"),
                        controller_role.resource_address()
                    ));
                    minter_updater => rule!(require_amount(
                        dec!("0.75"),
                        controller_role.resource_address()
                    ));
                ))
                .burn_roles(burn_roles!(
                    burner => rule!(require(global_caller(component_address))
                    || require_amount(
                        dec!("0.75"),
                        controller_role.resource_address()
                    ));
                    burner_updater => rule!(require_amount(
                        dec!("0.75"),
                        controller_role.resource_address()
                    ));
                ))
                .create_with_no_initial_supply();

            //create the cdp marker manager
            let cdp_marker_manager: ResourceManager =
                ResourceBuilder::new_integer_non_fungible::<CdpMarker>(OwnerRole::Fixed(rule!(
                    require_amount(dec!("0.75"), controller_role.resource_address())
                )))
                .metadata(metadata!(
                    init {
                        "name" => "Stabilis Marker Receipt", locked;
                        "symbol" => "stabMARK", locked;
                        "description" => "A receipt received by marking a Stabilis loan", updatable;
                        "info_url" => "https://stabilis.finance", updatable;
                        "icon_url" => Url::of("https://i.imgur.com/Xi6nrsv.png"), updatable;
                    }
                ))
                .non_fungible_data_update_roles(non_fungible_data_update_roles!(
                    non_fungible_data_updater => rule!(require(global_caller(component_address))
                    || require_amount(
                        dec!("0.75"),
                        controller_role.resource_address()
                    ));
                    non_fungible_data_updater_updater => rule!(require_amount(
                        dec!("0.75"),
                        controller_role.resource_address()
                    ));
                ))
                .mint_roles(mint_roles!(
                    minter => rule!(require(global_caller(component_address))
                    || require_amount(
                        dec!("0.75"),
                        controller_role.resource_address()
                    ));
                    minter_updater => rule!(require_amount(
                        dec!("0.75"),
                        controller_role.resource_address()
                    ));
                ))
                .burn_roles(burn_roles!(
                    burner => rule!(require(global_caller(component_address))
                    || require_amount(
                        dec!("0.75"),
                        controller_role.resource_address()
                    ));
                    burner_updater => rule!(require_amount(
                        dec!("0.75"),
                        controller_role.resource_address()
                    ));
                ))
                .create_with_no_initial_supply();

            //create the liquidation receipt manager
            let liquidation_receipt_manager: ResourceManager =
                ResourceBuilder::new_integer_non_fungible::<LiquidationReceipt>(OwnerRole::Fixed(
                    rule!(require_amount(
                        dec!("0.75"),
                        controller_role.resource_address()
                    )),
                ))
                .metadata(metadata!(
                    init {
                        "name" => "Stabilis Liquidation Receipt", locked;
                        "symbol" => "stabLIQ", locked;
                        "description" => "A receipt received for liquidating a Stabilis Loan", updatable;
                        "info_url" => "https://stabilis.finance", updatable;
                        "icon_url" => Url::of("https://i.imgur.com/UnrCzEM.png"), updatable;
                    }
                ))
                .non_fungible_data_update_roles(non_fungible_data_update_roles!(
                    non_fungible_data_updater => rule!(require(global_caller(component_address))
                    || require_amount(dec!("0.75"),
                    controller_role.resource_address()
                    ));
                    non_fungible_data_updater_updater => rule!(require_amount(
                        dec!("0.75"),
                        controller_role.resource_address()
                    ));
                ))
                .mint_roles(mint_roles!(
                    minter => rule!(require(global_caller(component_address))
                    || require_amount(dec!("0.75"),
                    controller_role.resource_address()
                    ));
                    minter_updater => rule!(require_amount(
                        dec!("0.75"),
                        controller_role.resource_address()
                    ));
                ))
                .burn_roles(burn_roles!(
                    burner => rule!(allow_all);
                    burner_updater => rule!(deny_all);
                ))
                .create_with_no_initial_supply();

            //create the stabilis component
            let stabilis = Self {
                collaterals: KeyValueStore::<ResourceAddress, CollateralInfo>::new(),
                pool_units: KeyValueStore::<ResourceAddress, PoolUnitInfo>::new(),
                collateral_ratios: KeyValueStore::<
                    ResourceAddress,
                    AvlTree<Decimal, Vec<NonFungibleLocalId>>,
                >::new(),
                cdp_counter: 0,
                cdp_manager,
                stab_manager,
                controller_badge_manager,
                internal_stab_price: dec!(1),
                circulating_stab: dec!(0),
                cdp_marker_manager,
                cdp_marker_counter: 0,
                marked_cdps: AvlTree::new(),
                marked_cdps_active: 0,
                marker_placing_counter: dec!(0),
                liquidation_receipt_manager,
                liquidation_counter: 0,
                parameters,
            }
            .instantiate()
            .prepare_to_globalize(OwnerRole::Fixed(rule!(require_amount(
                dec!("0.75"),
                controller_role.resource_address()
            ))))
            .with_address(address_reservation)
            .globalize();

            // Return the component address as well as the controller badges
            (stabilis, controller_role)
        }

        pub fn open_cdp(
            &mut self,
            collateral: Bucket,
            stab_to_mint: Decimal,
            safe: bool,
        ) -> (Bucket, Bucket) {
            let mut is_pool_unit_collateral: bool = false;
            let stab_tokens: Bucket = self.stab_manager.mint(stab_to_mint);

            assert!(
                stab_tokens.amount() >= self.parameters.minimum_mint,
                "Minted STAB is less than the minimum required amount."
            );
            assert!(
                !self.parameters.stop_openings,
                "Not allowed to open loans right now."
            );

            //Check if collateral is accepted and if it is a pool unit
            if self
                .pool_units
                .get(&collateral.resource_address())
                .is_some()
            {
                assert!(
                    self.pool_units
                        .get(&collateral.resource_address())
                        .unwrap()
                        .accepted,
                    "This collateral is not accepted"
                );
                is_pool_unit_collateral = true;
            } else {
                assert!(
                    self.collaterals
                        .get(&collateral.resource_address())
                        .map(|c| c.accepted)
                        .unwrap_or(false),
                    "This collateral is not accepted"
                );
            }

            //Calculate collateral amount, if pool unit convert to real
            let collateral_amount: Decimal = self.pool_to_real(
                collateral.amount(),
                collateral.resource_address(),
                is_pool_unit_collateral,
            );

            //Assign parent address, this is equal to the collateral address unless the collateral is a pool unit
            let parent_collateral_address: ResourceAddress = match is_pool_unit_collateral {
                false => collateral.resource_address(),
                true => {
                    self.pool_units
                        .get(&collateral.resource_address())
                        .unwrap()
                        .parent_address
                }
            };

            self.collaterals
                .get_mut(&parent_collateral_address)
                .unwrap()
                .collateral_amount += collateral_amount;

            //Get collateral MCR
            let mcr: Decimal = self
                .collaterals
                .get(&parent_collateral_address)
                .unwrap()
                .mcr;

            //Assert that the collateral value is high enough
            if safe {
                assert!(
                    self.collaterals
                        .get(&parent_collateral_address)
                        .unwrap()
                        .usd_price
                        * collateral_amount
                        >= self.internal_stab_price * stab_tokens.amount() * mcr,
                    "Collateral value too low."
                );
            } else {
                assert!(
                    self.collaterals
                        .get(&parent_collateral_address)
                        .unwrap()
                        .usd_price
                        * collateral_amount
                        >= self.internal_stab_price * stab_tokens.amount() * dec!("0.75"),
                    "Collateral value too low."
                );
            }

            self.cdp_counter += 1;

            //calculate collateral ratio
            let cr: Decimal = collateral_amount / stab_tokens.amount();

            //Insert collateral ratio into AvlTree
            if self
                .collaterals
                .get(&parent_collateral_address)
                .unwrap()
                .initialized
            {
                self.insert_cr(
                    parent_collateral_address,
                    cr,
                    NonFungibleLocalId::integer(self.cdp_counter),
                );
            } else {
                let mut avl_tree: AvlTree<Decimal, Vec<NonFungibleLocalId>> = AvlTree::new();
                let cdp_ids: Vec<NonFungibleLocalId> =
                    vec![NonFungibleLocalId::integer(self.cdp_counter)];
                avl_tree.insert(cr, cdp_ids);
                self.collateral_ratios
                    .insert(parent_collateral_address, avl_tree);
                self.collaterals
                    .get_mut(&parent_collateral_address)
                    .unwrap()
                    .initialized = true;
                self.collaterals
                    .get_mut(&parent_collateral_address)
                    .unwrap()
                    .highest_cr = cr;
            }

            //Create Cdp struct for the receipt
            let cdp = Cdp {
                collateral: collateral.resource_address(),
                parent_address: parent_collateral_address,
                is_pool_unit_collateral,
                collateral_amount: collateral.amount(),
                minted_stab: stab_tokens.amount(),
                collateral_stab_ratio: cr,
                status: CdpStatus::Healthy,
                marker_id: 0u64,
            };

            //Check whether the share of this collateral's minted STAB is too high and update STAB circulating supply
            self.update_minted_stab(
                true,
                is_pool_unit_collateral,
                true,
                stab_tokens.amount(),
                parent_collateral_address,
                collateral.resource_address(),
            );

            //Mint the Cdp receipt
            let cdp_receipt: NonFungibleBucket = self
                .cdp_manager
                .mint_non_fungible(&NonFungibleLocalId::integer(self.cdp_counter), cdp)
                .as_non_fungible();

            //Store the collateral in the correct vault
            self.put_collateral(
                collateral.resource_address(),
                is_pool_unit_collateral,
                collateral,
            );

            //return the minted STAB and the Cdp receipt
            (stab_tokens, cdp_receipt.into())
        }

        pub fn close_cdp(
            &mut self,
            receipt_id: NonFungibleLocalId,
            mut stab_payment: Bucket,
        ) -> (Bucket, Bucket) {
            let receipt_data: Cdp = self.cdp_manager.get_non_fungible_data(&receipt_id);

            assert!(
                stab_payment.amount() >= receipt_data.minted_stab,
                "not enough STAB supplied to close completely"
            );
            assert!(
                !self.parameters.stop_closings,
                "Not allowed to close loans right now."
            );
            assert!(
                receipt_data.status == CdpStatus::Healthy,
                "Loan not healthy. Can't close right now. In case of liquidation, retrieve collateral. Else, add collateral to save."
            );
            assert!(
                stab_payment.resource_address() == self.stab_manager.address(),
                "Invalid STAB payment."
            );

            //Remove collateral from Vault
            let collateral: Bucket = self.take_collateral(
                receipt_data.collateral,
                receipt_data.is_pool_unit_collateral,
                receipt_data.collateral_amount,
            );

            self.collaterals
                .get_mut(&receipt_data.parent_address)
                .unwrap()
                .collateral_amount -= receipt_data.collateral_stab_ratio * receipt_data.minted_stab;

            //Update circulating STAB, both for total and chosen collateral
            self.update_minted_stab(
                false,
                receipt_data.is_pool_unit_collateral,
                false,
                receipt_data.minted_stab,
                receipt_data.parent_address,
                receipt_data.collateral,
            );

            //Burn the paid back STAB
            stab_payment.take(receipt_data.minted_stab).burn();

            //Remove the collateral ratio from the AvlTree
            self.remove_cr(
                receipt_data.parent_address,
                receipt_data.collateral_stab_ratio,
                receipt_id.clone(),
            );

            //Update the Cdp receipt
            self.cdp_manager
                .update_non_fungible_data(&receipt_id, "status", CdpStatus::Closed);

            self.cdp_manager
                .update_non_fungible_data(&receipt_id, "collateral_amount", dec!(0));

            //return the collateral and the leftover STAB
            (collateral, stab_payment)
        }

        pub fn retrieve_leftover_collateral(&mut self, receipt_id: NonFungibleLocalId) -> Bucket {
            let receipt_data: Cdp = self.cdp_manager.get_non_fungible_data(&receipt_id);

            assert!(
                receipt_data.status == CdpStatus::Liquidated
                    || receipt_data.status == CdpStatus::ForceLiquidated,
                "Loan not liquidated"
            );
            assert!(
                receipt_data.collateral_amount > dec!(0),
                "No collateral leftover"
            );
            assert!(
                !self.parameters.stop_closings,
                "Not allowed to close loans right now."
            );

            //Update Cdp receipt to 0 collateral
            self.cdp_manager
                .update_non_fungible_data(&receipt_id, "collateral_amount", dec!(0));

            //Return leftover collateral
            self.take_collateral(
                receipt_data.collateral,
                receipt_data.is_pool_unit_collateral,
                receipt_data.collateral_amount,
            )
        }

        pub fn top_up_cdp(&mut self, collateral_id: NonFungibleLocalId, collateral: Bucket) {
            let receipt_data: Cdp = self.cdp_manager.get_non_fungible_data(&collateral_id);
            let new_collateral_amount = receipt_data.collateral_amount + collateral.amount();

            assert!(
                receipt_data.status == CdpStatus::Healthy
                    || receipt_data.status == CdpStatus::Marked,
                "Loan not healthy or marked."
            );
            assert!(
                receipt_data.collateral == collateral.resource_address(),
                "Incompatible token."
            );

            //Remove the collateral ratio from the AvlTree
            if receipt_data.status == CdpStatus::Healthy {
                self.remove_cr(
                    receipt_data.parent_address,
                    receipt_data.collateral_stab_ratio,
                    collateral_id.clone(),
                );
            }

            //Calculate new collateral ratio
            let cr: Decimal = self.pool_to_real(
                new_collateral_amount,
                collateral.resource_address(),
                receipt_data.is_pool_unit_collateral,
            ) / receipt_data.minted_stab;

            self.collaterals
                .get_mut(&receipt_data.parent_address)
                .unwrap()
                .collateral_amount +=
                (cr - receipt_data.collateral_stab_ratio) * receipt_data.minted_stab;

            assert!(
                cr > self
                    .collaterals
                    .get(&receipt_data.parent_address)
                    .unwrap()
                    .liquidation_collateral_ratio,
                "Not enough collateral added to save this loan."
            );

            //Insert new collateral ratio into AvlTree
            self.insert_cr(receipt_data.parent_address, cr, collateral_id.clone());

            //Store the collateral in the correct vault
            self.put_collateral(
                receipt_data.collateral,
                receipt_data.is_pool_unit_collateral,
                collateral,
            );

            //Update the Cdp receipt
            self.cdp_manager
                .update_non_fungible_data(&collateral_id, "collateral_stab_ratio", cr);
            self.cdp_manager.update_non_fungible_data(
                &collateral_id,
                "collateral_amount",
                new_collateral_amount,
            );

            if receipt_data.status == CdpStatus::Marked {
                let marker_data: CdpMarker = self
                    .cdp_marker_manager
                    .get_non_fungible_data(&NonFungibleLocalId::integer(receipt_data.marker_id));
                self.cdp_manager.update_non_fungible_data(
                    &collateral_id,
                    "status",
                    CdpStatus::Healthy,
                );
                self.cdp_marker_manager.update_non_fungible_data(
                    &NonFungibleLocalId::integer(receipt_data.marker_id),
                    "used",
                    true,
                );
                self.marked_cdps.remove(&marker_data.marker_placing);
                self.marked_cdps_active -= 1;
            }
        }

        pub fn remove_collateral(
            &mut self,
            collateral_id: NonFungibleLocalId,
            amount: Decimal,
        ) -> Bucket {
            let receipt_data: Cdp = self.cdp_manager.get_non_fungible_data(&collateral_id);
            let new_collateral_amount = receipt_data.collateral_amount - amount;

            assert!(
                receipt_data.status == CdpStatus::Healthy,
                "Loan not healthy. Save it first."
            );

            //Remove the collateral ratio from the AvlTree
            self.remove_cr(
                receipt_data.parent_address,
                receipt_data.collateral_stab_ratio,
                collateral_id.clone(),
            );

            //Calculate new collateral ratio
            let cr: Decimal = self.pool_to_real(
                new_collateral_amount,
                receipt_data.collateral,
                receipt_data.is_pool_unit_collateral,
            ) / receipt_data.minted_stab;

            self.collaterals
                .get_mut(&receipt_data.parent_address)
                .unwrap()
                .collateral_amount +=
                (cr - receipt_data.collateral_stab_ratio) * receipt_data.minted_stab;

            //Insert new collateral ratio into AvlTree
            self.insert_cr(receipt_data.parent_address, cr, collateral_id.clone());

            assert!(
                cr > self
                    .collaterals
                    .get_mut(&receipt_data.parent_address)
                    .unwrap()
                    .liquidation_collateral_ratio,
                "Removal would put the CR below MCR."
            );

            //Retrieve the to-be returned collateral from the correct vault
            let removed_collateral: Bucket = self.take_collateral(
                receipt_data.collateral,
                receipt_data.is_pool_unit_collateral,
                amount,
            );

            //Update the Cdp receipt
            self.cdp_manager
                .update_non_fungible_data(&collateral_id, "collateral_stab_ratio", cr);
            self.cdp_manager.update_non_fungible_data(
                &collateral_id,
                "collateral_amount",
                new_collateral_amount,
            );

            //Return the removed collateral
            removed_collateral
        }

        pub fn partial_close_cdp(&mut self, collateral_id: NonFungibleLocalId, repayment: Bucket) {
            assert!(
                repayment.resource_address() == self.stab_manager.address(),
                "Invalid STAB payment."
            );
            let receipt_data: Cdp = self.cdp_manager.get_non_fungible_data(&collateral_id);
            let new_stab_amount = receipt_data.minted_stab - repayment.amount();

            assert!(
                new_stab_amount >= self.parameters.minimum_mint,
                "Resulting borrowed STAB needs to be above minimum mint."
            );

            assert!(
                receipt_data.status == CdpStatus::Healthy,
                "Loan not healthy. Save it first."
            );

            //Remove the collateral ratio from the AvlTree
            self.remove_cr(
                receipt_data.parent_address,
                receipt_data.collateral_stab_ratio,
                collateral_id.clone(),
            );

            //Calculate new collateral ratio
            let cr: Decimal = self.pool_to_real(
                receipt_data.collateral_amount,
                receipt_data.collateral,
                receipt_data.is_pool_unit_collateral,
            ) / new_stab_amount;

            self.update_minted_stab(
                false,
                receipt_data.is_pool_unit_collateral,
                false,
                repayment.amount(),
                receipt_data.parent_address,
                receipt_data.collateral,
            );

            repayment.burn();

            //Insert new collateral ratio into AvlTree
            self.insert_cr(receipt_data.parent_address, cr, collateral_id.clone());

            assert!(
                cr > self
                    .collaterals
                    .get_mut(&receipt_data.parent_address)
                    .unwrap()
                    .liquidation_collateral_ratio,
                "Action would put the CR below MCR."
            );

            //Update the Cdp receipt
            self.cdp_manager
                .update_non_fungible_data(&collateral_id, "collateral_stab_ratio", cr);
            self.cdp_manager.update_non_fungible_data(
                &collateral_id,
                "minted_stab",
                new_stab_amount,
            );
        }

        pub fn borrow_more(
            &mut self,
            collateral_id: NonFungibleLocalId,
            amount: Decimal,
        ) -> Bucket {
            let receipt_data: Cdp = self.cdp_manager.get_non_fungible_data(&collateral_id);
            let new_stab_amount = receipt_data.minted_stab + amount;

            assert!(
                receipt_data.status == CdpStatus::Healthy,
                "Loan not healthy. Save it first."
            );

            //Remove the collateral ratio from the AvlTree
            self.remove_cr(
                receipt_data.parent_address,
                receipt_data.collateral_stab_ratio,
                collateral_id.clone(),
            );

            //Calculate new collateral ratio
            let cr: Decimal = self.pool_to_real(
                receipt_data.collateral_amount,
                receipt_data.collateral,
                receipt_data.is_pool_unit_collateral,
            ) / new_stab_amount;

            self.update_minted_stab(
                true,
                receipt_data.is_pool_unit_collateral,
                true,
                amount,
                receipt_data.parent_address,
                receipt_data.collateral,
            );

            //Insert new collateral ratio into AvlTree
            self.insert_cr(receipt_data.parent_address, cr, collateral_id.clone());

            assert!(
                cr > self
                    .collaterals
                    .get_mut(&receipt_data.parent_address)
                    .unwrap()
                    .liquidation_collateral_ratio,
                "Removal would put the CR below MCR."
            );

            //Update the Cdp receipt
            self.cdp_manager
                .update_non_fungible_data(&collateral_id, "collateral_stab_ratio", cr);
            self.cdp_manager.update_non_fungible_data(
                &collateral_id,
                "minted_stab",
                new_stab_amount,
            );

            self.stab_manager.mint(amount)
        }

        pub fn mark_for_liquidation(&mut self, collateral: ResourceAddress) -> Bucket {
            //Get the CDP with lowest collateral ratio for the chosen collateral
            let (_first_cr, collateral_ids) = self
                .collateral_ratios
                .get_mut(&collateral)
                .unwrap()
                .range(dec!(0)..)
                .next()
                .unwrap();
            let collateral_id: NonFungibleLocalId = collateral_ids[0].clone();

            let data: Cdp = self.cdp_manager.get_non_fungible_data(&collateral_id);

            assert!(
                data.collateral_stab_ratio
                    < self
                        .collaterals
                        .get(&collateral)
                        .unwrap()
                        .liquidation_collateral_ratio,
                "No possible liquidations."
            );

            //Calculate new collateral ratio
            let cr: Decimal = self.pool_to_real(
                data.collateral_amount,
                data.collateral,
                data.is_pool_unit_collateral,
            ) / data.minted_stab;

            self.collaterals
                .get_mut(&data.parent_address)
                .unwrap()
                .collateral_amount += (cr - data.collateral_stab_ratio) * data.minted_stab;

            self.marker_placing_counter += dec!(1);
            self.cdp_marker_counter += 1;
            let id: Decimal = self.marker_placing_counter;

            //Create the marker receipt struct
            let marker = CdpMarker {
                mark_type: CdpUpdate::Marked,
                time_marked: Clock::current_time_rounded_to_minutes(),
                marked_id: collateral_id.clone(),
                marker_placing: self.marker_placing_counter,
                used: false,
            };

            //Insert the CDP into the marked CDPs AvlTree
            self.marked_cdps.insert(id, collateral_id.clone());
            self.marked_cdps_active += 1;

            //Remove the collateral ratio from the AvlTree
            self.remove_cr(
                data.parent_address,
                data.collateral_stab_ratio,
                collateral_id.clone(),
            );

            //Mint marker receipt, which will be returned if the marking is a success
            let marker_receipt_success: NonFungibleBucket = self
                .cdp_marker_manager
                .mint_non_fungible(
                    &NonFungibleLocalId::integer(self.cdp_marker_counter),
                    marker,
                )
                .as_non_fungible();

            //Update the Cdp receipt to point to the marker receipt and get marked status
            self.cdp_manager.update_non_fungible_data(
                &collateral_id,
                "marker_id",
                self.cdp_marker_counter,
            );
            self.cdp_manager
                .update_non_fungible_data(&collateral_id, "status", CdpStatus::Marked);

            //Get the collateral ids for the collateral ratio that was newly calculated (which is different if working with pool units)
            let mut cdp_ids: Vec<NonFungibleLocalId> = Vec::new();

            if self
                .collateral_ratios
                .get_mut(&collateral)
                .unwrap()
                .get_mut(&cr)
                .is_some()
            {
                cdp_ids = self
                    .collateral_ratios
                    .get_mut(&collateral)
                    .unwrap()
                    .get_mut(&cr)
                    .unwrap()
                    .to_vec();
            }

            let return_receipt: Bucket;

            //Save CDP if CR is high enough again after staking reward update, unless the collateral_ids vector is full
            //Return the initial marker receipt if saving wasn't possible
            //Or return a new marker receipt if saving was possible
            if (cr
                > self
                    .collaterals
                    .get(&collateral)
                    .unwrap()
                    .liquidation_collateral_ratio)
                && cdp_ids.len() < self.parameters.max_vector_length.try_into().unwrap()
            {
                let marker_data: CdpMarker = marker_receipt_success.non_fungible().data();
                marker_receipt_success.burn();
                let cdp_data: Cdp = self.cdp_manager.get_non_fungible_data(&collateral_id);

                return_receipt = self.save(marker_data, cdp_data, cr);
            } else {
                return_receipt = marker_receipt_success.into();
            }

            return_receipt
        }

        pub fn force_liquidate(
            &mut self,
            collateral: ResourceAddress,
            mut payment: Bucket,
            percentage_to_take: Decimal,
            assert_non_markable: bool,
        ) -> (Bucket, Bucket) {
            assert!(
                !self.parameters.stop_force_liquidate,
                "Not allowed to forceliquidate loans right now."
            );

            assert!(
                payment.resource_address() == self.stab_manager.address(),
                "Invalid STAB payment."
            );

            //Get the CDP with lowest collateral ratio for the chosen collateral
            let (_first_cr, collateral_ids) = self
                .collateral_ratios
                .get_mut(&collateral)
                .unwrap()
                .range(dec!(0)..)
                .next()
                .unwrap();
            let collateral_id: NonFungibleLocalId = collateral_ids[0].clone();
            let data: Cdp = self.cdp_manager.get_non_fungible_data(&collateral_id);

            //Remove the collateral ratio from the AvlTree
            self.remove_cr(
                data.parent_address,
                data.collateral_stab_ratio,
                collateral_id.clone(),
            );

            //Calculate latest collateral ratio
            let cr: Decimal = self.pool_to_real(
                data.collateral_amount,
                data.collateral,
                data.is_pool_unit_collateral,
            ) / data.minted_stab;

            //get liquidation collateral ratio
            let lcr: Decimal = self
                .collaterals
                .get(&collateral)
                .unwrap()
                .liquidation_collateral_ratio;

            //assert that the collateral ratio is high enough to force liquidate:
            //if cr < lcr the loan doesn't have to be forced, but can be liquidated via normal means.
            if assert_non_markable {
                assert!(
                    cr > lcr,
                    "CR is too low. Liquidate this loan via the normal procedure."
                );
            }

            //calculate percentage of collateral value vs. minted stab
            let cr_percentage: Decimal = self.collaterals.get(&collateral).unwrap().mcr * cr / lcr;

            //calculate how much of the loan can be liquidated, how much of the payment should be taken, and what the leftover stab debt will then be
            let (percentage_to_liquidate, payment_amount, new_stab_amount): (
                Decimal,
                Decimal,
                Decimal,
            ) = match payment.amount() > data.minted_stab {
                true => (dec!(1), data.minted_stab, dec!(0)),
                false => (
                    (payment.amount() / data.minted_stab),
                    payment.amount(),
                    data.minted_stab - payment.amount(),
                ),
            };

            //calculate new collateral amount
            let mut new_collateral_amount: Decimal = data.collateral_amount
                - (data.collateral_amount * percentage_to_liquidate * percentage_to_take
                    / cr_percentage);

            //if cr is too low, not all collateral the liquidator wants to take can be taken, so the new_collateral_amount will be negative
            //then we set the new_collateral_amount to 0
            if new_collateral_amount < dec!(0) {
                new_collateral_amount = dec!(0);
            }

            //if cr percentage is not > 100%, the entire loan must be liquidated
            //otherwise we can be left with a loan that has debt but 0 collateral
            assert!(
                cr_percentage > dec!(1) || percentage_to_liquidate == dec!(1),
                "CR < 100%. Entire loan must be liquidated",
            );

            //take the payment and burn the STAB
            payment.take(payment_amount).burn();

            //Update circulating STAB
            self.update_minted_stab(
                false,
                data.is_pool_unit_collateral,
                false,
                payment_amount,
                data.parent_address,
                data.collateral,
            );

            let collateral_payment: Bucket = self.take_collateral(
                data.collateral,
                data.is_pool_unit_collateral,
                data.collateral_amount - new_collateral_amount,
            );

            //set this again, because of rounding in take_collateral method
            new_collateral_amount = data.collateral_amount - collateral_payment.amount();

            //Update the Cdp receipt
            self.cdp_manager.update_non_fungible_data(
                &collateral_id,
                "collateral_amount",
                new_collateral_amount,
            );
            self.cdp_manager.update_non_fungible_data(
                &collateral_id,
                "minted_stab",
                new_stab_amount,
            );

            //If the new collateral amount is not 0, calculate the new collateral ratio, insert it into the AvlTree and update the Cdp receipt
            if percentage_to_liquidate < dec!(1) {
                let new_cr: Decimal = self.pool_to_real(
                    new_collateral_amount,
                    data.collateral,
                    data.is_pool_unit_collateral,
                ) / new_stab_amount;

                self.collaterals
                    .get_mut(&data.parent_address)
                    .unwrap()
                    .collateral_amount += (new_cr - data.collateral_stab_ratio) * data.minted_stab;

                self.insert_cr(data.parent_address, new_cr, collateral_id.clone());

                self.cdp_manager.update_non_fungible_data(
                    &collateral_id,
                    "collateral_stab_ratio",
                    new_cr,
                );
            } else {
                //If the entire loan was liquidated, update the Cdp receipt to reflect this
                self.cdp_manager.update_non_fungible_data(
                    &collateral_id,
                    "status",
                    CdpStatus::ForceLiquidated,
                );

                self.collaterals
                    .get_mut(&data.parent_address)
                    .unwrap()
                    .collateral_amount -= data.collateral_stab_ratio * data.minted_stab;
            }

            //return the payment and the leftover STAB
            (collateral_payment, payment)
        }

        pub fn force_mint(
            &mut self,
            collateral: ResourceAddress,
            mut payment: Bucket,
            percentage_to_supply: Decimal,
        ) -> (Bucket, Option<Bucket>) {
            assert!(
                !self.parameters.stop_force_mint,
                "Not allowed to force mint right now."
            );

            let mut data: Option<Cdp> = None;
            let mut collateral_id: NonFungibleLocalId = NonFungibleLocalId::integer(0);
            let mut return_bucket: Option<Bucket> = None;

            {
                //Get the CDP with highest collateral ratio for the chosen collateral
                let collateral_ratios = self.collateral_ratios.get_mut(&collateral).unwrap();
                let range = collateral_ratios.range_back(
                    dec!(0)..(self.collaterals.get(&collateral).unwrap().highest_cr + dec!(1)),
                );

                'outer_loop: for (_cr, collateral_ids) in range {
                    for found_collateral_id in collateral_ids {
                        data = Some(self.cdp_manager.get_non_fungible_data(&found_collateral_id));
                        if data.as_ref().unwrap().collateral == payment.resource_address() {
                            collateral_id = found_collateral_id.clone();
                            break 'outer_loop;
                        }
                    }
                }
            }

            let data = data.expect("No suitable mints found");
            assert!(
                data.collateral == payment.resource_address(),
                "Can only force mint other collaterals right now."
            );

            let pool_to_real: Decimal =
                self.pool_to_real(dec!(1), data.collateral, data.is_pool_unit_collateral);

            //calculate minimum allowed collateral ratio
            let min_collateral_ratio: Decimal = self.parameters.force_mint_cr_multiplier
                * self
                    .collaterals
                    .get(&collateral)
                    .unwrap()
                    .liquidation_collateral_ratio;

            //get collateral price
            let collateral_price: Decimal = self.collaterals.get(&collateral).unwrap().usd_price;

            //calculate constant k, which is the collateral needed for minting 1 STAB
            let k: Decimal = (self.internal_stab_price) / (pool_to_real * collateral_price)
                * percentage_to_supply;

            //we now need to calculate maximum amount of collateral that can be supplied: max_addition
            //we can do this by first claiming: collateral_amount / stab_amount = min_collateral_ratio (1)
            //collateral_amount = (initial_collateral_amount + max_col_addition) * pool_to_real (2)
            //stab_amount = initial_stab_amount + max_col_addition / k (3)
            //filling in (2) and (3) in (1) gives us an equation of the form: ((c + a) * p) / (s + a / k) = m (4)
            //solving (4) for max_col_addition (abbreviated 'a') gives: a = (k * (c * p - m * s)) / (m - k * p)
            //which translates to:

            let max_addition: Decimal = (k
                * (data.collateral_amount * pool_to_real
                    - min_collateral_ratio * data.minted_stab))
                / (min_collateral_ratio - k * pool_to_real);

            //if too much collateral is supplied, remove the excess and put in bucket to return
            //note RoundingMode is set to round away from zero
            //if we take too little collateral by rounding down (to zero), we will go below the minimum collateral ratio, by providing too much collateral to mint STAB with. So it's better to return too much.
            if payment.amount() > max_addition {
                return_bucket = Some(payment.take_advanced(
                    payment.amount() - max_addition,
                    WithdrawStrategy::Rounded(RoundingMode::AwayFromZero),
                ));
            }

            //Remove the current collateral ratio from the AvlTree
            self.remove_cr(
                data.parent_address,
                data.collateral_stab_ratio,
                collateral_id.clone(),
            );

            //calculate newly minted stab, new collateral amount and new collateral ratio
            let new_minted_stab: Decimal = data.minted_stab + payment.amount() / k;
            let new_collateral_amount: Decimal = data.collateral_amount + payment.amount();

            let new_cr: Decimal = self.pool_to_real(
                new_collateral_amount,
                data.collateral,
                data.is_pool_unit_collateral,
            ) / new_minted_stab;

            self.collaterals
                .get_mut(&data.parent_address)
                .unwrap()
                .collateral_amount += (new_cr - data.collateral_stab_ratio) * data.minted_stab;

            //Update the Cdp receipt accordingly
            self.cdp_manager.update_non_fungible_data(
                &collateral_id,
                "minted_stab",
                new_minted_stab,
            );
            self.cdp_manager.update_non_fungible_data(
                &collateral_id,
                "collateral_amount",
                new_collateral_amount,
            );
            self.cdp_manager.update_non_fungible_data(
                &collateral_id,
                "collateral_stab_ratio",
                new_cr,
            );

            //Insert the new collateral ratio into the AvlTree
            self.insert_cr(data.parent_address, new_cr, collateral_id.clone());

            //Mint the STAB
            let stab_tokens: Bucket = self.stab_manager.mint(payment.amount() / k);

            //Update circulating STAB
            self.update_minted_stab(
                false,
                data.is_pool_unit_collateral,
                false,
                stab_tokens.amount(),
                data.parent_address,
                data.collateral,
            );

            //put the collateral in the correct vault
            self.put_collateral(data.collateral, data.is_pool_unit_collateral, payment);

            //return the minted STAB and the leftover collateral
            (stab_tokens, return_bucket)
        }

        pub fn liquidate_position_with_marker(
            &mut self,
            marker_id: NonFungibleLocalId,
            payment: Bucket,
        ) -> (Bucket, Option<Bucket>, Bucket) {
            assert!(
                payment.resource_address() == self.stab_manager.address(),
                "Invalid STAB payment."
            );
            let marker_data: CdpMarker = self.cdp_marker_manager.get_non_fungible_data(&marker_id);

            let cdp_data: Cdp = self
                .cdp_manager
                .get_non_fungible_data(&marker_data.marked_id);

            //Try to liquidate the CDP, returns liquidation rewards or receipt for saving the CDP
            self.try_liquidate(
                payment,
                cdp_data,
                marker_data,
                marker_id,
                self.parameters.liquidation_delay,
            )
        }

        pub fn liquidate_position_without_marker(
            &mut self,
            payment: Bucket,
            automatic: bool,
            skip: i64,
            cdp_id: NonFungibleLocalId,
        ) -> (Bucket, Option<Bucket>, Bucket) {
            assert!(
                payment.resource_address() == self.stab_manager.address(),
                "Invalid STAB payment."
            );
            //Finding the next to-be liquidated CDP, skipping over the amount of CDPs specified by the skip parameter
            let mut collateral_id: NonFungibleLocalId = cdp_id;
            let mut skip_counter: i64 = 0;
            let mut found: bool = false;

            if automatic {
                for (_identifier, found_collateral_id) in self.marked_cdps.range(dec!(0)..) {
                    collateral_id = found_collateral_id.clone();
                    skip_counter += 1;
                    if (skip_counter - 1) == skip {
                        found = true;
                        break;
                    }
                }
                if skip_counter == 0 {
                    panic!("No loans available to liquidate.");
                } else if !found {
                    panic!(
                        "Too many skipped. Skip a maximum of {} loans.",
                        skip_counter - 1
                    );
                }
            }

            let cdp_data: Cdp = self.cdp_manager.get_non_fungible_data(&collateral_id);
            let marker_data: CdpMarker = self
                .cdp_marker_manager
                .get_non_fungible_data(&NonFungibleLocalId::integer(cdp_data.marker_id));

            let marker_id: NonFungibleLocalId = NonFungibleLocalId::integer(cdp_data.marker_id);

            //Try to liquidate the CDP, returns liquidation rewards or receipt for saving the CDP
            self.try_liquidate(
                payment,
                cdp_data,
                marker_data,
                marker_id,
                self.parameters.liquidation_delay + self.parameters.unmarked_delay,
            )
        }

        pub fn change_collateral_price(&mut self, collateral: ResourceAddress, new_price: Decimal) {
            let mcr: Decimal = self.collaterals.get_mut(&collateral).unwrap().mcr;
            self.collaterals.get_mut(&collateral).unwrap().usd_price = new_price;
            self.collaterals
                .get_mut(&collateral)
                .unwrap()
                .liquidation_collateral_ratio = mcr * (self.internal_stab_price / new_price);
        }

        pub fn add_collateral(
            &mut self,
            address: ResourceAddress,
            chosen_mcr: Decimal,
            initial_price: Decimal,
        ) {
            assert!(
                self.collaterals.get(&address).is_none(),
                "Collateral is already accepted."
            );

            let info = CollateralInfo {
                mcr: chosen_mcr,
                usd_price: initial_price,
                liquidation_collateral_ratio: chosen_mcr * self.internal_stab_price / initial_price,
                vault: Vault::new(address),
                resource_address: address,
                treasury: Vault::new(address),
                accepted: true,
                initialized: false,
                max_stab_share: dec!(1),
                minted_stab: dec!(0),
                collateral_amount: dec!(0),
                highest_cr: dec!(0),
            };

            self.collaterals.insert(address, info);
        }

        pub fn add_pool_collateral(
            &self,
            address: ResourceAddress,
            parent_address: ResourceAddress,
            pool_address: ComponentAddress,
            lsu: bool,
            initial_acceptance: bool,
        ) {
            assert!(
                self.pool_units.get(&address).is_none(),
                "Collateral is already accepted."
            );

            let mut validator: Option<Global<Validator>> = None;
            let mut one_resource_pool: Option<Global<OneResourcePool>> = None;

            if lsu {
                validator = Some(Global::from(pool_address));
            } else {
                one_resource_pool = Some(Global::from(pool_address));
            }

            let info = PoolUnitInfo {
                vault: Vault::new(address),
                treasury: Vault::new(address),
                lsu,
                validator,
                one_resource_pool,
                parent_address,
                address,
                accepted: initial_acceptance,
                max_pool_share: dec!(1),
                minted_stab: dec!(0),
            };

            self.pool_units.insert(address, info);
        }

        pub fn change_internal_price(&mut self, new_price: Decimal) {
            self.internal_stab_price = new_price;
        }

        //emptying the treasury of a collateral, error_fallback exists if a pool unit is also in self.collaterals
        pub fn empty_collateral_treasury(
            &mut self,
            amount: Decimal,
            collateral: ResourceAddress,
            error_fallback: bool,
        ) -> Bucket {
            if self.pool_units.get(&collateral).is_some() && !error_fallback {
                return self
                    .pool_units
                    .get_mut(&collateral)
                    .unwrap()
                    .treasury
                    .take_advanced(amount, WithdrawStrategy::Rounded(RoundingMode::ToZero));
            } else {
                return self
                    .collaterals
                    .get_mut(&collateral)
                    .unwrap()
                    .treasury
                    .take_advanced(amount, WithdrawStrategy::Rounded(RoundingMode::ToZero));
            }
        }

        pub fn mint_controller_badge(&self, amount: Decimal) -> Bucket {
            self.controller_badge_manager.mint(amount)
        }

        pub fn edit_collateral(
            &mut self,
            address: ResourceAddress,
            new_mcr: Decimal,
            new_acceptance: bool,
            new_max_share: Decimal,
        ) {
            self.collaterals.get_mut(&address).unwrap().accepted = new_acceptance;
            self.collaterals.get_mut(&address).unwrap().mcr = new_mcr;
            self.collaterals.get_mut(&address).unwrap().max_stab_share = new_max_share;
        }

        pub fn edit_pool_collateral(
            &mut self,
            address: ResourceAddress,
            new_acceptance: bool,
            new_max_share: Decimal,
        ) {
            self.pool_units.get_mut(&address).unwrap().accepted = new_acceptance;
            self.pool_units.get_mut(&address).unwrap().max_pool_share = new_max_share;
        }

        pub fn set_liquidation_delay(&mut self, new_delay: i64) {
            self.parameters.liquidation_delay = new_delay;
        }

        pub fn set_unmarked_delay(&mut self, new_delay: i64) {
            self.parameters.unmarked_delay = new_delay;
        }

        pub fn set_stops(
            &mut self,
            liquidations: bool,
            openings: bool,
            closings: bool,
            force_mint: bool,
            force_liquidate: bool,
        ) {
            self.parameters.stop_closings = closings;
            self.parameters.stop_liquidations = liquidations;
            self.parameters.stop_openings = openings;
            self.parameters.stop_force_liquidate = force_liquidate;
            self.parameters.stop_force_mint = force_mint;
        }

        pub fn set_max_vector_length(&mut self, new_max_length: u64) {
            self.parameters.max_vector_length = new_max_length;
        }

        pub fn set_minimum_mint(&mut self, new_minimum_mint: Decimal) {
            self.parameters.minimum_mint = new_minimum_mint;
        }

        pub fn set_fines(&mut self, liquidator_fine: Decimal, stabilis_fine: Decimal) {
            self.parameters.liquidation_liquidation_fine = liquidator_fine;
            self.parameters.stabilis_liquidation_fine = stabilis_fine;
        }

        pub fn set_force_mint_multiplier(&mut self, new_multiplier: Decimal) {
            self.parameters.force_mint_cr_multiplier = new_multiplier;
        }

        pub fn return_internal_price(&self) -> Decimal {
            self.internal_stab_price
        }

        pub fn free_stab(&mut self, amount: Decimal) -> Bucket {
            self.stab_manager.mint(amount)
        }

        pub fn burn_stab(&mut self, bucket: Bucket) {
            assert!(
                bucket.resource_address() == self.stab_manager.address(),
                "Can only burn STAB, not another token."
            );
            bucket.burn();
        }

        pub fn burn_marker(&self, marker: Bucket) {
            let data: CdpMarker = marker.as_non_fungible().non_fungible().data();
            assert!(
                self.cdp_marker_manager.address() == marker.resource_address(),
                "Can only burn markers, not another token."
            );
            assert!(data.used, "Only used markers can be burned!");
            marker.burn();
        }

        pub fn burn_loan_receipt(&self, receipt: Bucket) {
            let data: Cdp = receipt.as_non_fungible().non_fungible().data();
            assert!(
                self.cdp_manager.address() == receipt.resource_address(),
                "Can only burn loan receipts, not another token."
            );
            assert!(
                data.status == CdpStatus::Liquidated
                    || data.status == CdpStatus::ForceLiquidated
                    || data.status == CdpStatus::Closed,
                "Loan not closed or liquidated"
            );
            assert!(
                data.collateral_amount == dec!(0),
                "Retrieve all collateral before burning!"
            );
            receipt.burn();
        }

        //HELPER METHODS

        fn try_liquidate(
            &mut self,
            payment: Bucket,
            cdp_data: Cdp,
            marker_data: CdpMarker,
            marker_id: NonFungibleLocalId,
            delay: i64,
        ) -> (Bucket, Option<Bucket>, Bucket) {
            //get cr where liquidation can occur
            let liquidation_collateral_ratio = self
                .collaterals
                .get(&cdp_data.parent_address)
                .unwrap()
                .liquidation_collateral_ratio;

            //assert liquidation is currently enabled, the marker is valid, the payment is sufficient, the time has passed, and the loan is marked
            assert!(
                !self.parameters.stop_liquidations,
                "Not allowed to liquidate loans right now."
            );
            assert!(
                !marker_data.used && marker_data.mark_type == CdpUpdate::Marked,
                "Non-valid marker."
            );
            assert!(
                payment.amount() >= cdp_data.minted_stab,
                "not enough STAB supplied to close completely"
            );

            assert!(
                Clock::current_time_is_at_or_after(
                    marker_data.time_marked.add_minutes(delay).unwrap(),
                    TimePrecision::Minute
                ),
                "Not yet able to liquidate."
            );

            assert!(cdp_data.status == CdpStatus::Marked, "Loan not marked");

            //get newest cr
            let cr: Decimal = self.pool_to_real(
                cdp_data.collateral_amount,
                cdp_data.collateral,
                cdp_data.is_pool_unit_collateral,
            ) / cdp_data.minted_stab;

            //check whether cr is sufficient, liquidate if not, save if it is
            if cr < liquidation_collateral_ratio {
                let (liquidation_payment, remainder, receipt): (Bucket, Bucket, Bucket) =
                    self.liquidate(payment, marker_data, marker_id, cdp_data, cr);
                (liquidation_payment, Some(remainder), receipt)
            } else {
                let marker_receipt: Bucket = self.save(marker_data, cdp_data, cr);
                (payment, None, marker_receipt)
            }
        }

        fn liquidate(
            &mut self,
            mut payment: Bucket,
            marker_data: CdpMarker,
            marker_id: NonFungibleLocalId,
            cdp_data: Cdp,
            cr: Decimal,
        ) -> (Bucket, Bucket, Bucket) {
            //update minted stab
            self.update_minted_stab(
                false,
                cdp_data.is_pool_unit_collateral,
                false,
                cdp_data.minted_stab,
                cdp_data.parent_address,
                cdp_data.collateral,
            );

            self.collaterals
                .get_mut(&cdp_data.parent_address)
                .unwrap()
                .collateral_amount -= cdp_data.collateral_stab_ratio * cdp_data.minted_stab;

            //set some variables that will be used, and create liq. receipt structure
            let mcr: Decimal = self.collaterals.get(&cdp_data.parent_address).unwrap().mcr;
            let liq_cr: Decimal = self
                .collaterals
                .get(&cdp_data.parent_address)
                .unwrap()
                .liquidation_collateral_ratio;
            let mut treasury_payment_amount: Option<Decimal> = None;
            let liquidation_payment_amount;
            let mut liquidation_receipt = LiquidationReceipt {
                collateral: cdp_data.collateral,
                stab_paid: cdp_data.minted_stab,
                percentage_owed: dec!(1) + self.parameters.liquidation_liquidation_fine,
                percentage_received: dec!(1) + self.parameters.liquidation_liquidation_fine,
                cdp_liquidated: marker_data.marked_id.clone(),
                date_liquidated: Clock::current_time_rounded_to_minutes(),
            };

            self.liquidation_counter += 1;

            //update the marker and cdp receipts
            self.marked_cdps.remove(&marker_data.marker_placing);
            self.marked_cdps_active -= 1;
            self.cdp_marker_manager
                .update_non_fungible_data(&marker_id, "used", true);
            self.cdp_manager.update_non_fungible_data(
                &marker_data.marked_id,
                "status",
                CdpStatus::Liquidated,
            );

            //take the payment, check whether it's enough, and burn it
            assert!(
                cdp_data.minted_stab <= payment.amount(),
                "Not enough STAB to liquidate."
            );
            let repayment: Bucket = payment.take(cdp_data.minted_stab);
            repayment.burn();

            //calculate the cr percentage, just the cr in percentage of the minted stab value
            //example: collateral value is $100, minted stab value is $80 -> cr = 100/80 = 1.25
            let cr_percentage: Decimal = mcr * cr / liq_cr;

            //calculate liquidations depending on cr
            //sit 1: cr > 1 + liquidation fine + stabilis fine   -> everyone can receive complete fines
            //sit 2: cr > 1 + liquidation fine                   -> liquidator receives whole fine, stabilis a partial fine
            //sit 3: cr <= 1                                     -> liquidator receives whole collateral, which might be less than minted stab

            if cr_percentage
                > dec!(1)
                    + self.parameters.liquidation_liquidation_fine
                    + self.parameters.stabilis_liquidation_fine
            {
                if self.parameters.stabilis_liquidation_fine > dec!(0) {
                    treasury_payment_amount = Some(
                        (self.parameters.stabilis_liquidation_fine)
                            * (cdp_data.collateral_amount / cr_percentage),
                    );
                }
                liquidation_payment_amount = (dec!(1)
                    + self.parameters.liquidation_liquidation_fine)
                    * (cdp_data.collateral_amount / cr_percentage);
            } else if cr_percentage > dec!(1) + self.parameters.liquidation_liquidation_fine {
                liquidation_payment_amount = (dec!(1)
                    + self.parameters.liquidation_liquidation_fine)
                    * (cdp_data.collateral_amount / cr_percentage);

                treasury_payment_amount =
                    Some(cdp_data.collateral_amount - liquidation_payment_amount);
            } else {
                liquidation_receipt.percentage_received = cr_percentage;
                liquidation_payment_amount = cdp_data.collateral_amount;
            }

            //make liq. receipt
            let receipt: NonFungibleBucket = self
                .liquidation_receipt_manager
                .mint_non_fungible(
                    &NonFungibleLocalId::integer(self.liquidation_counter),
                    liquidation_receipt,
                )
                .as_non_fungible();

            //handle calculated liquidations
            let treasury_payment = if let Some(payment_amount) = treasury_payment_amount {
                Some(self.take_collateral(
                    cdp_data.collateral,
                    cdp_data.is_pool_unit_collateral,
                    payment_amount,
                ))
            } else {
                None
            };

            let liquidation_payment = self.take_collateral(
                cdp_data.collateral,
                cdp_data.is_pool_unit_collateral,
                liquidation_payment_amount,
            );

            let leftover_collateral: Decimal = cdp_data.collateral_amount
                - liquidation_payment.amount()
                - treasury_payment
                    .as_ref()
                    .map_or(dec!(0), |payment_bucket| payment_bucket.amount());

            //update liquidated cdp
            self.cdp_manager.update_non_fungible_data(
                &marker_data.marked_id,
                "collateral_amount",
                leftover_collateral,
            );

            if let Some(payment) = treasury_payment {
                self.put_collateral_in_treasury(
                    cdp_data.collateral,
                    cdp_data.is_pool_unit_collateral,
                    payment,
                );
            }

            (liquidation_payment, payment, receipt.into())
        }

        fn save(&mut self, marker_data: CdpMarker, cdp_data: Cdp, cr: Decimal) -> Bucket {
            self.collaterals
                .get_mut(&cdp_data.parent_address)
                .unwrap()
                .collateral_amount += (cr - cdp_data.collateral_stab_ratio) * cdp_data.minted_stab;

            //create marker for savior
            self.marker_placing_counter += dec!(1);
            self.cdp_marker_counter += 1;

            let marker = CdpMarker {
                mark_type: CdpUpdate::Saved,
                time_marked: Clock::current_time_rounded_to_minutes(),
                marked_id: marker_data.marked_id.clone(),
                marker_placing: self.marker_placing_counter,
                used: false,
            };

            let marker_receipt: NonFungibleBucket = self
                .cdp_marker_manager
                .mint_non_fungible(
                    &NonFungibleLocalId::integer(self.cdp_marker_counter),
                    marker,
                )
                .as_non_fungible();

            //update state to healthy cdp, both marked_cdps and the cdp itself
            self.marked_cdps.remove(&marker_data.marker_placing);
            self.marked_cdps_active -= 1;
            self.cdp_manager.update_non_fungible_data(
                &marker_data.marked_id,
                "status",
                CdpStatus::Healthy,
            );
            self.cdp_manager.update_non_fungible_data(
                &marker_data.marked_id,
                "collateral_stab_ratio",
                cr,
            );
            self.cdp_marker_manager.update_non_fungible_data(
                &NonFungibleLocalId::integer(cdp_data.marker_id),
                "used",
                true,
            );

            //insert the healthy cdp again
            self.insert_cr(cdp_data.parent_address, cr, marker_data.marked_id.clone());

            marker_receipt.into()
        }

        fn insert_cr(
            &mut self,
            parent_address: ResourceAddress,
            cr: Decimal,
            cdp_id: NonFungibleLocalId,
        ) {
            if self
                .collateral_ratios
                .get_mut(&parent_address)
                .unwrap()
                .get_mut(&cr)
                .is_some()
            {
                let mut cdp_ids: Vec<NonFungibleLocalId> = self
                    .collateral_ratios
                    .get_mut(&parent_address)
                    .unwrap()
                    .get_mut(&cr)
                    .unwrap()
                    .clone()
                    .to_vec();
                assert!(
                    cdp_ids.len() < self.parameters.max_vector_length.try_into().unwrap(),
                    "CR vector is full..."
                );
                cdp_ids.push(cdp_id);
                self.collateral_ratios
                    .get_mut(&parent_address)
                    .unwrap()
                    .insert(cr, cdp_ids);
            } else {
                let cdp_ids: Vec<NonFungibleLocalId> = vec![cdp_id];
                self.collateral_ratios
                    .get_mut(&parent_address)
                    .unwrap()
                    .insert(cr, cdp_ids);
            }

            if self.collaterals.get(&parent_address).unwrap().highest_cr < cr {
                self.collaterals
                    .get_mut(&parent_address)
                    .unwrap()
                    .highest_cr = cr;
            }
        }

        fn remove_cr(
            &mut self,
            parent_address: ResourceAddress,
            cr: Decimal,
            receipt_id: NonFungibleLocalId,
        ) {
            let mut collateral_ids: Vec<NonFungibleLocalId> = self
                .collateral_ratios
                .get_mut(&parent_address)
                .unwrap()
                .get_mut(&cr)
                .unwrap()
                .to_vec();

            collateral_ids.retain(|id| id != &receipt_id);

            self.collateral_ratios
                .get_mut(&parent_address)
                .unwrap()
                .insert(cr, collateral_ids.clone());

            if collateral_ids.is_empty() {
                self.collateral_ratios
                    .get_mut(&parent_address)
                    .unwrap()
                    .remove(&cr);
            }
        }

        fn pool_to_real(
            &mut self,
            amount: Decimal,
            collateral: ResourceAddress,
            pool: bool,
        ) -> Decimal {
            if pool {
                if self.pool_units.get_mut(&collateral).unwrap().lsu {
                    self.pool_units
                        .get_mut(&collateral)
                        .unwrap()
                        .validator
                        .unwrap()
                        .get_redemption_value(amount)
                } else {
                    self.pool_units
                        .get_mut(&collateral)
                        .unwrap()
                        .one_resource_pool
                        .unwrap()
                        .get_redemption_value(amount)
                }
            } else {
                amount
            }
        }

        fn check_share(
            &mut self,
            parent_collateral_address: ResourceAddress,
            is_pool_unit_collateral: bool,
            collateral_address: ResourceAddress,
        ) {
            assert!(
                self.collaterals
                    .get(&parent_collateral_address)
                    .unwrap()
                    .minted_stab
                    / self.circulating_stab
                    <= self
                        .collaterals
                        .get(&parent_collateral_address)
                        .unwrap()
                        .max_stab_share,
                "This collateral's share is too big already"
            );
            if is_pool_unit_collateral {
                assert!(
                    self.pool_units
                        .get(&collateral_address)
                        .unwrap()
                        .minted_stab
                        / self
                            .collaterals
                            .get(&parent_collateral_address)
                            .unwrap()
                            .minted_stab
                        <= self
                            .pool_units
                            .get(&collateral_address)
                            .unwrap()
                            .max_pool_share,
                    "This pool collateral's share is too big already"
                );
            }
        }

        fn update_minted_stab(
            &mut self,
            add: bool,
            is_pool_unit_collateral: bool,
            check_share: bool,
            amount: Decimal,
            collateral: ResourceAddress,
            pool_unit: ResourceAddress,
        ) {
            if add {
                self.collaterals.get_mut(&collateral).unwrap().minted_stab += amount;
                if is_pool_unit_collateral {
                    self.pool_units.get_mut(&pool_unit).unwrap().minted_stab += amount;
                }
                self.circulating_stab += amount;
            } else {
                self.collaterals.get_mut(&collateral).unwrap().minted_stab -= amount;
                if is_pool_unit_collateral {
                    self.pool_units.get_mut(&pool_unit).unwrap().minted_stab -= amount;
                }
                self.circulating_stab -= amount;
            }

            if check_share {
                self.check_share(collateral, is_pool_unit_collateral, pool_unit);
            }
        }

        fn take_collateral(
            &mut self,
            collateral: ResourceAddress,
            pool: bool,
            amount: Decimal,
        ) -> Bucket {
            if pool {
                self.pool_units
                    .get_mut(&collateral)
                    .unwrap()
                    .vault
                    .take_advanced(amount, WithdrawStrategy::Rounded(RoundingMode::ToZero))
            } else {
                self.collaterals
                    .get_mut(&collateral)
                    .unwrap()
                    .vault
                    .take_advanced(amount, WithdrawStrategy::Rounded(RoundingMode::ToZero))
            }
        }

        fn put_collateral(
            &mut self,
            collateral: ResourceAddress,
            pool: bool,
            collateral_bucket: Bucket,
        ) -> &mut KeyValueStore<ResourceAddress, CollateralInfo> {
            if pool {
                self.pool_units
                    .get_mut(&collateral)
                    .unwrap()
                    .vault
                    .put(collateral_bucket)
            } else {
                self.collaterals
                    .get_mut(&collateral)
                    .unwrap()
                    .vault
                    .put(collateral_bucket)
            }

            &mut self.collaterals
        }

        fn put_collateral_in_treasury(
            &mut self,
            collateral: ResourceAddress,
            pool: bool,
            collateral_bucket: Bucket,
        ) -> &mut KeyValueStore<ResourceAddress, CollateralInfo> {
            if pool {
                self.pool_units
                    .get_mut(&collateral)
                    .unwrap()
                    .treasury
                    .put(collateral_bucket)
            } else {
                self.collaterals
                    .get_mut(&collateral)
                    .unwrap()
                    .treasury
                    .put(collateral_bucket)
            }

            &mut self.collaterals
        }
    }
}
