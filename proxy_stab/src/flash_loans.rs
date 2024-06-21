use crate::structs::*;
use scrypto::prelude::*;

#[blueprint]
mod flash_loans {
    enable_method_auth! {
        methods {
            borrow => restrict_to: [OWNER];
            settings => restrict_to: [OWNER];
            pay_back => restrict_to: [OWNER];
            retrieve_interest => restrict_to: [OWNER];
        }
    }

    const STABILIS: Global<Stabilis> = global_component!(
        Stabilis,
        "component_tdx_2_1cp4j27fcr4e74g59euhje8wk4u4q0jq358jf8u4j4z7znfqnj6jx0q"
    );

    extern_blueprint! {
        "package_tdx_2_1pkqn52t324ezshectwlwkzk0w8zqamyq6aqugkmq9q7zcvhshfq0s2",
        Stabilis {
            fn free_stab(&self, amount: Decimal) -> Bucket;
            fn burn_stab(&self, bucket: Bucket) -> ();
        }
    }

    struct FlashLoans {
        badge_vault: FungibleVault,
        loan_receipt_manager: ResourceManager,
        interest_vault: Option<Vault>,
        loan_receipt_counter: u64,
        interest: Decimal,
        stabilis: Global<Stabilis>,
        enabled: bool,
    }

    impl FlashLoans {
        pub fn instantiate(controller_badge: Bucket) -> Global<FlashLoans> {
            let (address_reservation, component_address) =
                Runtime::allocate_component_address(FlashLoans::blueprint_id());

            let loan_receipt_manager: ResourceManager =
                ResourceBuilder::new_integer_non_fungible::<LoanReceipt>(OwnerRole::Fixed(rule!(
                    require_amount(dec!("0.75"), controller_badge.resource_address())
                )))
                .metadata(metadata!(
                    init {
                        "name" => "STAB Flash Loan Receipt", locked;
                        "symbol" => "stabFLASH", locked;
                        "description" => "A receipt for your STAB flash loan", locked;
                        "info_url" => "https://stabilis.finance", updatable;
                    }
                ))
                .non_fungible_data_update_roles(non_fungible_data_update_roles!(
                    non_fungible_data_updater => rule!(require(global_caller(component_address)));
                    non_fungible_data_updater_updater => rule!(deny_all);
                ))
                .mint_roles(mint_roles!(
                    minter => rule!(require(global_caller(component_address)));
                    minter_updater => rule!(deny_all);
                ))
                .burn_roles(burn_roles!(
                    burner => rule!(require(global_caller(component_address)));
                    burner_updater => rule!(deny_all);
                ))
                .deposit_roles(deposit_roles!(
                    depositor => rule!(deny_all);
                    depositor_updater => rule!(deny_all);
                ))
                .create_with_no_initial_supply();

            let controller_address: ResourceAddress = controller_badge.resource_address();

            //create the flash loan component
            Self {
                badge_vault: FungibleVault::with_bucket(controller_badge.as_fungible()),
                loan_receipt_manager,
                interest: dec!(0),
                interest_vault: None,
                stabilis: STABILIS,
                loan_receipt_counter: 0,
                enabled: true,
            }
            .instantiate()
            .prepare_to_globalize(OwnerRole::Fixed(rule!(require(controller_address))))
            .with_address(address_reservation)
            .globalize()
        }

        pub fn settings(&mut self, interest: Decimal, enabled: bool) {
            self.interest = interest;
            self.enabled = enabled;
        }

        pub fn borrow(&mut self, amount: Decimal) -> (Bucket, Bucket) {
            assert!(self.enabled, "Flash loans are disabled.");
            let loan_receipt = LoanReceipt {
                borrowed_amount: amount,
                interest: self.interest,
            };

            let receipt: Bucket = self.loan_receipt_manager.mint_non_fungible(
                &NonFungibleLocalId::integer(self.loan_receipt_counter),
                loan_receipt,
            );
            self.loan_receipt_counter += 1;

            let loan_bucket: Bucket = self
                .badge_vault
                .authorize_with_amount(dec!("0.75"), || self.stabilis.free_stab(amount));

            (loan_bucket, receipt)
        }

        pub fn pay_back(&mut self, receipt_bucket: Bucket, mut payment: Bucket) -> Bucket {
            assert!(
                receipt_bucket.resource_address() == self.loan_receipt_manager.address(),
                "Invalid receipt"
            );

            let receipt: LoanReceipt = self
                .loan_receipt_manager
                .get_non_fungible_data(&receipt_bucket.as_non_fungible().non_fungible_local_id());

            assert!(
                payment.amount() >= receipt.borrowed_amount * (dec!(1) + receipt.interest),
                "Not enough STAB paid back."
            );

            self.badge_vault.authorize_with_amount(dec!("0.75"), || {
                self.stabilis
                    .burn_stab(payment.take(receipt.borrowed_amount))
            });

            if receipt.interest > dec!(0) {
                if self.interest_vault.is_none() {
                    self.interest_vault = Some(Vault::with_bucket(
                        payment.take(receipt.interest * receipt.borrowed_amount),
                    ));
                } else {
                    self.interest_vault
                        .as_mut()
                        .unwrap()
                        .put(payment.take(receipt.interest * receipt.borrowed_amount));
                }
            }

            receipt_bucket.burn();

            payment
        }

        pub fn retrieve_interest(&mut self) -> Bucket {
            self.interest_vault.as_mut().unwrap().take_all()
        }
    }
}
