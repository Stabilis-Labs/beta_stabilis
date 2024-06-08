use crate::structs::*;
use crate::staking::staking::*;
use scrypto::prelude::*;

#[blueprint]
mod governance {
    enable_method_auth! {
        methods {
            put_ilis => PUBLIC;
            receive_badges => PUBLIC;
            send_badges => restrict_to: [OWNER];
            mint_dao_badges => restrict_to: [OWNER];
            send_ilis => restrict_to: [OWNER];
            pay => restrict_to: [OWNER];
            send_pay => restrict_to: [OWNER];
            set_parameters => restrict_to: [OWNER];
            post_announcement => restrict_to: [OWNER];
            remove_announcement => restrict_to: [OWNER];
            set_update_reward => restrict_to: [OWNER];
            set_proxy_component => restrict_to: [OWNER];
            create_proposal => PUBLIC;
            add_proposal_step => PUBLIC;
            submit_proposal => PUBLIC;
            vote_on_proposal => PUBLIC;
            finish_voting => PUBLIC;
            execute_proposal_step => PUBLIC;
            retrieve_fee => PUBLIC;
            rewarded_update => PUBLIC;
        }
    }

    struct Governance {
        staking: Global<Staking>,
        ilis_vault: Vault,
        proposal_fee_vault: Vault,
        proposal_receipt_manager: ResourceManager,
        dao_owner_badge_manager: ResourceManager,
        badge_vaults: KeyValueStore<ResourceAddress, Vault>,
        payments: KeyValueStore<ComponentAddress, Decimal>,
        incomplete_proposals: KeyValueStore<u64, Proposal>,
        ongoing_proposals: KeyValueStore<u64, Proposal>,
        rejected_proposals: KeyValueStore<u64, Proposal>,
        accepted_proposals: KeyValueStore<u64, Proposal>,
        finished_proposals: KeyValueStore<u64, Proposal>,
        text_announcements: KeyValueStore<u64, String>,
        text_announcement_counter: u64,
        proposal_counter: u64,
        parameters: GovernanceParameters,
        voting_id_manager: ResourceManager,
        last_staking_update: Instant,
        daily_update_reward: Decimal,
        proxy: Global<AnyComponent>,
    }

    impl Governance {
        pub fn instantiate_dao(
            founder_allocation: Decimal,
            voting_id_address: ResourceAddress,
            controller_badge: Bucket,
            proxy_component: ComponentAddress,
        ) -> (Global<Governance>, Bucket, Bucket) {
            let (address_reservation, component_address) =
                Runtime::allocate_component_address(Governance::blueprint_id());

            let dao_owner: FungibleBucket = ResourceBuilder::new_fungible(OwnerRole::None)
                .divisibility(DIVISIBILITY_MAXIMUM)
                .metadata(metadata! (
                    init {
                        "name" => "Owner badge Stabilis DAO component", locked;
                        "symbol" => "stabDAO", locked;
                    }
                ))
                .mint_roles(mint_roles!(
                    minter => rule!(require(global_caller(component_address)));
                    minter_updater => rule!(deny_all);
                ))
                .mint_initial_supply(1);

            let mut ilis_bucket: Bucket = ResourceBuilder::new_fungible(OwnerRole::Fixed(rule!(
                require(dao_owner.resource_address())
            )))
            .divisibility(DIVISIBILITY_MAXIMUM)
            .metadata(metadata! (
                init {
                    "name" => "ILIS token", locked;
                    "symbol" => "ILIS", locked;
                    "description" => "Token tied to the Stabilis protocol", locked;
                    "icon_url" => Url::of("https://imgur.com/jZMMzUu.png"), updatable;
                }
            ))
            .mint_roles(mint_roles!(
                minter => rule!(require_amount(
                    dec!("0.75"),
                    dao_owner.resource_address()
                ));
                minter_updater => rule!(deny_all);
            ))
            .burn_roles(burn_roles!(
                burner => rule!(allow_all);
                burner_updater => rule!(deny_all);
            ))
            .mint_initial_supply(100_000_000)
            .into();

            let proposal_receipt_manager = ResourceBuilder::new_integer_non_fungible::<
                ProposalReceipt,
            >(OwnerRole::Fixed(rule!(require(
                dao_owner.resource_address()
            ))))
            .metadata(metadata!(
                init {
                    "name" => "Stabilis Proposal Receipt", locked;
                    "symbol" => "stabPROP", locked;
                    "description" => "A receipt proving ownership of a Stabilis proposal", locked;
                    "icon_url" => Url::of("https://i.imgur.com/ugeqSMF.png"), updatable;
                }
            ))
            .mint_roles(mint_roles!(
                minter => rule!(require(global_caller(component_address))
                || require_amount(
                    dec!("0.75"),
                    dao_owner.resource_address()
                ));
                minter_updater => rule!(deny_all);
            ))
            .burn_roles(burn_roles!(
                burner => rule!(deny_all);
                burner_updater => rule!(deny_all);
            ))
            .non_fungible_data_update_roles(non_fungible_data_update_roles!(
                non_fungible_data_updater => rule!(require(global_caller(component_address))
                || require_amount(
                    dec!("0.75"),
                    dao_owner.resource_address()));
                non_fungible_data_updater_updater => rule!(deny_all);
            ))
            .create_with_no_initial_supply();

            let staking: Global<Staking> = Staking::new(controller_badge.resource_address(), ilis_bucket.take(dec!(10_000_000)).as_fungible(), 1, "Stabilis".to_string(), "STAB".to_string(), true, 31);
            staking.add_stakable(
                ilis_bucket.resource_address(),
                dec!(10000),
                Lock {
                    payment: dec!(0),
                    duration: 0, 
                }
            );

            let parameters = GovernanceParameters {
                fee: dec!(10000),
                proposal_duration: 1,
                quorum: dec!(10000),
                minimum_for_fraction: dec!("0.5"),
            };

            let badge_vaults: KeyValueStore<ResourceAddress, Vault> = KeyValueStore::new();
            badge_vaults.insert(
                controller_badge.resource_address(),
                Vault::with_bucket(controller_badge),
            );

            let dao = Self {
                staking,
                proposal_fee_vault: Vault::new(ilis_bucket.resource_address()),
                ilis_vault: Vault::with_bucket(
                    ilis_bucket.take(ilis_bucket.amount() - founder_allocation)
                ),
                proposal_receipt_manager,
                dao_owner_badge_manager: ResourceManager::from(dao_owner.resource_address()),
                badge_vaults,
                payments: KeyValueStore::new(),
                incomplete_proposals: KeyValueStore::new(),
                ongoing_proposals: KeyValueStore::new(),
                rejected_proposals: KeyValueStore::new(),
                accepted_proposals: KeyValueStore::new(),
                finished_proposals: KeyValueStore::new(),
                text_announcements: KeyValueStore::new(),
                text_announcement_counter: 0,
                proposal_counter: 0,
                parameters,
                voting_id_manager: ResourceManager::from(voting_id_address),
                last_staking_update: Clock::current_time_rounded_to_minutes(),
                daily_update_reward: dec!(10000),
                proxy: Global::from(proxy_component),
            }
            .instantiate()
            .prepare_to_globalize(OwnerRole::Fixed(rule!(require(
                dao_owner.resource_address()
            ))))
            .with_address(address_reservation)
            .globalize();

            (dao, ilis_bucket, dao_owner.into())
        }

        pub fn put_ilis(&mut self, ilis_tokens: Bucket) {
            self.ilis_vault.put(ilis_tokens);
        }

        pub fn receive_badges(&mut self, badge_bucket: Bucket) {
            if self
                .badge_vaults
                .get_mut(&badge_bucket.resource_address())
                .is_some()
            {
                self.badge_vaults
                    .get_mut(&badge_bucket.resource_address())
                    .unwrap()
                    .put(badge_bucket);
            } else {
                self.badge_vaults.insert(
                    badge_bucket.resource_address(),
                    Vault::with_bucket(badge_bucket),
                );
            };
        }

        pub fn send_badges(
            &mut self,
            address: ResourceAddress,
            amount: Decimal,
            receiver_address: ComponentAddress,
        ) {
            let receiver: Global<AnyComponent> = Global::from(receiver_address);
            let badge_bucket: Bucket = self.badge_vaults.get_mut(&address).unwrap().take(amount);
            receiver.call_raw::<()>("receive_badges", scrypto_args!(badge_bucket));
        }

        pub fn send_ilis(&mut self, amount: Decimal, receiver_address: ComponentAddress) {
            let receiver: Global<AnyComponent> = Global::from(receiver_address);
            let ilis_bucket: Bucket = self.ilis_vault.take(amount);
            receiver.call_raw::<()>("put_ilis", scrypto_args!(ilis_bucket));
        }

        pub fn mint_dao_badges(&mut self, amount: Decimal) {
            self.badge_vaults
                .get_mut(&self.dao_owner_badge_manager.address())
                .unwrap()
                .put(self.dao_owner_badge_manager.mint(amount));
        }

        pub fn pay(&mut self, amount: Decimal, receiver: ComponentAddress) {
            self.payments.insert(receiver, amount);
        }

        pub fn send_pay(&mut self, account_address: ComponentAddress) {
            let mut receiver = Global::<Account>::from(account_address);
            let payment: Bucket = self
                .ilis_vault
                .take(*self.payments.get(&account_address).unwrap());
            receiver.try_deposit_or_abort(payment, None);
            self.payments.remove(&account_address);
        }

        pub fn set_parameters(
            &mut self,
            fee: Decimal,
            proposal_duration: i64,
            quorum: Decimal,
            minimum_for_fraction: Decimal,
        ) {
            self.parameters.fee = fee;
            self.parameters.proposal_duration = proposal_duration;
            self.parameters.quorum = quorum;
            self.parameters.minimum_for_fraction = minimum_for_fraction;
        }

        pub fn post_announcement(&mut self, announcement: String) {
            self.text_announcements
                .insert(self.text_announcement_counter, announcement);
            self.text_announcement_counter += 1;
        }

        pub fn remove_announcement(&mut self, announcement_id: u64) {
            self.text_announcements.remove(&announcement_id);
        }

        pub fn create_proposal(
            &mut self,
            title_description: (String, String),
            component: ComponentAddress,
            badge: ResourceAddress,
            method: String,
            args: ScryptoValue,
            mut payment: Bucket,
        ) -> (Bucket, Bucket) {
            assert!(
                payment.resource_address() == self.ilis_vault.resource_address()
                    && payment.amount() > self.parameters.fee,
                "Invalid payment, must be ILIS and more than the fee."
            );

            self.proposal_fee_vault
                .put(payment.take(self.parameters.fee));

            let first_step = ProposalStep {
                component,
                badge,
                method,
                args,
            };

            let proposal = Proposal {
                title: title_description.0,
                description: title_description.1,
                steps: vec![first_step],
                votes_for: dec!(0),
                votes_against: dec!(0),
                votes: KeyValueStore::new(),
                deadline: Clock::current_time_rounded_to_minutes()
                    .add_minutes(self.parameters.proposal_duration * 24 * 60)
                    .unwrap(),
                next_index: 0,
                status: ProposalStatus::Building,
            };

            let proposal_receipt = ProposalReceipt {
                fee_paid: self.parameters.fee,
                proposal_id: self.proposal_counter,
                status: ProposalStatus::Building,
            };

            let incomplete_proposal_receipt: Bucket =
                self.proposal_receipt_manager.mint_non_fungible(
                    &NonFungibleLocalId::integer(self.proposal_counter),
                    proposal_receipt,
                );

            self.incomplete_proposals
                .insert(self.proposal_counter, proposal);
            self.proposal_counter += 1;

            (payment, incomplete_proposal_receipt)
        }

        pub fn add_proposal_step(
            &mut self,
            proposal_receipt_proof: NonFungibleProof,
            component: ComponentAddress,
            badge: ResourceAddress,
            method: String,
            args: ScryptoValue,
        ) {
            let receipt_proof = proposal_receipt_proof.check_with_message(
                self.voting_id_manager.address(),
                "Invalid proposal receipt supplied!",
            );

            let receipt = receipt_proof.non_fungible::<ProposalReceipt>().data();
            let proposal_id: u64 = receipt.proposal_id;

            let mut proposal = self.incomplete_proposals.get_mut(&proposal_id).unwrap();

            let step = ProposalStep {
                component,
                badge,
                method,
                args,
            };

            proposal.steps.push(step);
        }

        pub fn submit_proposal(&mut self, proposal_receipt_proof: NonFungibleProof) {
            let receipt_proof = proposal_receipt_proof.check_with_message(
                self.proposal_receipt_manager.address(),
                "Invalid proposal receipt supplied!",
            );

            let receipt = receipt_proof.non_fungible::<ProposalReceipt>().data();
            let proposal_id: u64 = receipt.proposal_id;

            let mut proposal = self.incomplete_proposals.remove(&proposal_id).unwrap();

            proposal.status = ProposalStatus::Ongoing;
            let update: ProposalStatus = ProposalStatus::Ongoing;
            proposal.deadline = Clock::current_time_rounded_to_minutes()
                .add_minutes(self.parameters.proposal_duration * 24 * 60)
                .unwrap();

            self.proposal_receipt_manager.update_non_fungible_data(
                &NonFungibleLocalId::integer(proposal_id),
                "status",
                update,
            );

            self.ongoing_proposals.insert(proposal_id, proposal);
        }

        pub fn vote_on_proposal(
            &mut self,
            proposal_id: u64,
            for_against: bool,
            voting_id_proof: NonFungibleProof,
        ) {
            let id_proof = voting_id_proof.check_with_message(
                self.voting_id_manager.address(),
                "Invalid proposal receipt supplied!",
            );

            let id: NonFungibleLocalId = id_proof.non_fungible::<Id>().local_id().clone();
            let id_data = id_proof.non_fungible::<Id>().data();

            let mut proposal = self.ongoing_proposals.get_mut(&proposal_id).unwrap();

            assert!(
                !Clock::current_time_is_at_or_after(proposal.deadline, TimePrecision::Minute),
                "Voting period has passed!"
            );
            assert!(
                proposal.votes.get(&id).is_none(),
                "Already voted on this proposal!"
            );

            if for_against {
                proposal.votes.insert(id.clone(), id_data.resources.get(&self.ilis_vault.resource_address()).unwrap().amount_staked);
                proposal.votes_for += id_data.resources.get(&self.ilis_vault.resource_address()).unwrap().amount_staked;
            } else {
                proposal
                    .votes
                    .insert(id.clone(), dec!("-1") * id_data.resources.get(&self.ilis_vault.resource_address()).unwrap().amount_staked);
                proposal.votes_against += id_data.resources.get(&self.ilis_vault.resource_address()).unwrap().amount_staked;
            }

            self.staking.set_lock(self.ilis_vault.resource_address(), proposal.deadline, id.clone());
        }

        pub fn finish_voting(&mut self, proposal_id: u64) {
            let mut proposal = self.ongoing_proposals.remove(&proposal_id).unwrap();
            let fee_paid: Decimal = self
                .proposal_receipt_manager
                .get_non_fungible_data::<ProposalReceipt>(&NonFungibleLocalId::integer(proposal_id))
                .fee_paid;

            assert!(
                Clock::current_time_is_at_or_after(proposal.deadline, TimePrecision::Minute),
                "Voting period has not passed!"
            );

            let mut decision: ProposalStatus = ProposalStatus::Accepted;
            let total_votes = proposal.votes_for + proposal.votes_against;

            if total_votes < self.parameters.quorum {
                proposal.status = ProposalStatus::Rejected;
                self.rejected_proposals.insert(proposal_id, proposal);
                decision = ProposalStatus::Rejected;
                self.ilis_vault.put(self.proposal_fee_vault.take(fee_paid));
            } else if proposal.votes_for > self.parameters.minimum_for_fraction * total_votes {
                proposal.status = ProposalStatus::Accepted;
                self.accepted_proposals.insert(proposal_id, proposal);
            } else {
                proposal.status = ProposalStatus::Rejected;
                self.rejected_proposals.insert(proposal_id, proposal);
                decision = ProposalStatus::Rejected;
                self.ilis_vault.put(self.proposal_fee_vault.take(fee_paid));
            }
            self.proposal_receipt_manager.update_non_fungible_data(
                &NonFungibleLocalId::integer(proposal_id),
                "status",
                decision,
            );
        }

        pub fn execute_proposal_step(&mut self, proposal_id: u64, steps_to_execute: i64) {
            let mut proposal = self.accepted_proposals.remove(&proposal_id).unwrap();

            for _ in 0..steps_to_execute {
                let step: &ProposalStep = &proposal.steps[proposal.next_index as usize];
                let component: Global<AnyComponent> = Global::from(step.component);
                component.call::<ScryptoValue, ()>(&step.method, &step.args);
                proposal.next_index += 1;

                if proposal.next_index as usize == proposal.steps.len() {
                    break;
                }
            }
            if proposal.next_index as usize == proposal.steps.len() {
                proposal.status = ProposalStatus::Executed;
                self.finished_proposals.insert(proposal_id, proposal);
                self.proposal_receipt_manager.update_non_fungible_data(
                    &NonFungibleLocalId::integer(proposal_id),
                    "status",
                    ProposalStatus::Executed,
                );
            } else {
                self.accepted_proposals.insert(proposal_id, proposal);
            }
        }

        pub fn retrieve_fee(&mut self, proposal_receipt_proof: NonFungibleProof) -> Bucket {
            let receipt_proof = proposal_receipt_proof.check_with_message(
                self.proposal_receipt_manager.address(),
                "Invalid proposal receipt supplied!",
            );
            let receipt = receipt_proof.non_fungible::<ProposalReceipt>().data();

            assert!(
                receipt.status == ProposalStatus::Executed,
                "Only executed proposals can have their fees refunded!"
            );

            self.proposal_receipt_manager.update_non_fungible_data(
                receipt_proof.non_fungible::<ProposalReceipt>().local_id(),
                "status",
                ProposalStatus::Finished,
            );

            self.proposal_fee_vault.take(receipt.fee_paid)
        }

        pub fn rewarded_update(&mut self) -> Bucket {
            let passed_minutes: Decimal = (Clock::current_time_rounded_to_minutes()
                .seconds_since_unix_epoch
                - self.last_staking_update.seconds_since_unix_epoch)
                / dec!(60);

            self.proxy.call_raw::<()>("update", scrypto_args!());
            self.staking.update_period();
            self.last_staking_update = Clock::current_time_rounded_to_minutes();

            self.ilis_vault
            .take(
                (passed_minutes * self.daily_update_reward)
                    / (dec!(24) * dec!(60)),
            )
            .into()
        }

        pub fn set_proxy_component(&mut self, proxy_component: ComponentAddress) {
            self.proxy = Global::from(proxy_component);
        }

        pub fn set_update_reward(&mut self, reward: Decimal) {
            self.daily_update_reward = reward;
        }
    }
}
