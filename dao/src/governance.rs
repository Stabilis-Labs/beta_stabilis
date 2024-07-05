use crate::bootstrap::bootstrap::*;
use crate::staking::staking::*;
use crate::structs::*;
use scrypto::prelude::*;

#[blueprint]
mod governance {
    enable_method_auth! {
        methods {
            put_tokens => PUBLIC;
            send_tokens => restrict_to: [OWNER];
            employ => restrict_to: [OWNER];
            fire => restrict_to: [OWNER];
            airdrop_tokens => restrict_to: [OWNER];
            airdrop_staked_tokens => restrict_to: [OWNER];
            set_parameters => restrict_to: [OWNER];
            post_announcement => restrict_to: [OWNER];
            remove_announcement => restrict_to: [OWNER];
            set_update_reward => restrict_to: [OWNER];
            set_proxy_component => restrict_to: [OWNER];
            set_staking_component => restrict_to: [OWNER];
            send_salary_to_employee => PUBLIC;
            create_proposal => PUBLIC;
            add_proposal_step => PUBLIC;
            submit_proposal => PUBLIC;
            vote_on_proposal => PUBLIC;
            finish_voting => PUBLIC;
            execute_proposal_step => PUBLIC;
            retrieve_fee => PUBLIC;
            rewarded_update => PUBLIC;
            finish_bootstrap => restrict_to: [OWNER];
        }
    }

    struct Governance {
        staking: Global<Staking>,
        bootstrap: Global<LinearBootstrapPool>,
        mother_token_address: ResourceAddress,
        proposal_fee_vault: Vault,
        proposal_receipt_manager: ResourceManager,
        vaults: KeyValueStore<ResourceAddress, Vault>,
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
        voting_id_address: ResourceAddress,
        last_staking_update: Instant,
        daily_update_reward: Decimal,
        proxy: Global<AnyComponent>,
        controller_badge_address: ResourceAddress,
        payment_locker: Global<AccountLocker>,
        employees: KeyValueStore<Global<Account>, (Job, Instant)>,
    }

    impl Governance {
        pub fn instantiate_dao(
            founder_allocation: Decimal,
            controller_badge: Bucket,
            proxy_component: ComponentAddress,
            protocol_name: String,
            protocol_token_supply: Decimal,
            protocol_token_symbol: String,
            protocol_token_icon_url: Url,
            proposal_receipt_icon_url: Url,
            bootstrap_resource1: Bucket,
            bootstrap_resource2: Bucket,
        ) -> (Global<Governance>, Bucket, Option<Bucket>, Bucket) {
            let (address_reservation, component_address) =
                Runtime::allocate_component_address(Governance::blueprint_id());

            let payment_locker = Blueprint::<AccountLocker>::instantiate(
                OwnerRole::Fixed(rule!(require_amount(
                    dec!("0.75"),
                    controller_badge.resource_address()
                ))),
                rule!(require_amount(
                    dec!("0.75"),
                    controller_badge.resource_address()
                )),
                rule!(require_amount(
                    dec!("0.75"),
                    controller_badge.resource_address()
                )),
                rule!(require_amount(
                    dec!("0.75"),
                    controller_badge.resource_address()
                )),
                rule!(require_amount(
                    dec!("0.75"),
                    controller_badge.resource_address()
                )),
                None,
            );

            let mut mother_token_bucket: Bucket = ResourceBuilder::new_fungible(OwnerRole::Fixed(
                rule!(require(controller_badge.resource_address())),
            ))
            .divisibility(DIVISIBILITY_MAXIMUM)
            .metadata(metadata! (
                init {
                    "name" => format!("{} token", protocol_token_symbol), updatable;
                    "symbol" => format!("{}", protocol_token_symbol), updatable;
                    "description" => format!("Token tied to the {}", protocol_name), updatable;
                    "icon_url" => protocol_token_icon_url, updatable; //https://imgur.com/jZMMzUu.png"
                }
            ))
            .mint_roles(mint_roles!(
                minter => rule!(require_amount(
                    dec!("0.75"),
                    controller_badge.resource_address()
                ));
                minter_updater => rule!(deny_all);
            ))
            .burn_roles(burn_roles!(
                burner => rule!(allow_all);
                burner_updater => rule!(deny_all);
            ))
            .mint_initial_supply(protocol_token_supply)
            .into();

            let mother_token_address: ResourceAddress = mother_token_bucket.resource_address();

            let proposal_receipt_manager = ResourceBuilder::new_integer_non_fungible::<
                ProposalReceipt,
            >(OwnerRole::Fixed(rule!(require(
                controller_badge.resource_address()
            ))))
            .metadata(metadata!(
                init {
                    "name" => format!("{} proposal receipt", protocol_name), updatable;
                    "symbol" => format!("prop{}", protocol_token_symbol), updatable;
                    "description" => format!("Proposal receipt for {}", protocol_name), updatable;
                    "icon_url" => proposal_receipt_icon_url, updatable;
                }
            ))
            .mint_roles(mint_roles!(
                minter => rule!(require(global_caller(component_address))
                || require_amount(
                    dec!("0.75"),
                    controller_badge.resource_address()
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
                    controller_badge.resource_address()));
                non_fungible_data_updater_updater => rule!(deny_all);
            ))
            .create_with_no_initial_supply();

            let (staking, voting_id_address): (Global<Staking>, ResourceAddress) = Staking::new(
                controller_badge.resource_address(),
                mother_token_bucket.take(dec!(10_000_000)).as_fungible(),
                1,
                protocol_name,
                protocol_token_symbol,
                true,
                31,
            );

            let (bootstrap, no_bucket, bootstrap_badge): (
                Global<LinearBootstrapPool>,
                Option<Bucket>,
                Bucket,
            ) = LinearBootstrapPool::new(
                bootstrap_resource1,
                bootstrap_resource2,
                dec!("0.99"),
                dec!("0.01"),
                dec!("0.5"),
                dec!("0.5"),
                dec!("0.002"),
                7,
            );

            controller_badge.authorize_with_all(|| {
                staking.add_stakable(
                    mother_token_address, //stakable resource
                    dec!(0),              //reward amount
                    dec!("1.0005"),       //lock payment
                    365,                  //max lock duration
                    dec!(4),              //unlock multiplier
                );
            });

            let parameters = GovernanceParameters {
                fee: dec!(10000),
                proposal_duration: 1,
                quorum: dec!(10000),
                approval_threshold: dec!("0.5"),
            };

            let controller_badge_address: ResourceAddress = controller_badge.resource_address();

            let vaults: KeyValueStore<ResourceAddress, Vault> = KeyValueStore::new();

            vaults.insert(
                controller_badge.resource_address(),
                Vault::with_bucket(controller_badge),
            );

            vaults.insert(
                mother_token_address,
                Vault::with_bucket(
                    mother_token_bucket
                        .take(mother_token_bucket.amount() * (dec!(1) - founder_allocation)),
                ),
            );

            let dao = Self {
                payment_locker,
                staking,
                bootstrap,
                mother_token_address,
                proposal_fee_vault: Vault::new(mother_token_address),
                vaults,
                proposal_receipt_manager,
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
                voting_id_address,
                last_staking_update: Clock::current_time_rounded_to_minutes(),
                daily_update_reward: dec!(10000),
                proxy: Global::from(proxy_component),
                controller_badge_address,
                employees: KeyValueStore::new(),
            }
            .instantiate()
            .prepare_to_globalize(OwnerRole::Fixed(rule!(require(controller_badge_address))))
            .with_address(address_reservation)
            .globalize();

            (dao, mother_token_bucket, no_bucket, bootstrap_badge)
        }

        pub fn finish_bootstrap(&mut self) {
            self.put_tokens(self.bootstrap.finish_bootstrap());
        }

        pub fn put_tokens(&mut self, tokens: Bucket) {
            if self.vaults.get_mut(&tokens.resource_address()).is_some() {
                self.vaults
                    .get_mut(&tokens.resource_address())
                    .unwrap()
                    .put(tokens);
            } else {
                self.vaults
                    .insert(tokens.resource_address(), Vault::with_bucket(tokens));
            };
        }

        pub fn send_tokens(
            &mut self,
            address: ResourceAddress,
            tokens: ResourceSpecifier,
            receiver_address: ComponentAddress,
        ) {
            let payment: Bucket = match tokens {
                ResourceSpecifier::Fungible(amount) => self
                    .vaults
                    .get_mut(&address)
                    .unwrap()
                    .as_fungible()
                    .take(amount)
                    .into(),
                ResourceSpecifier::NonFungible(ids) => self
                    .vaults
                    .get_mut(&address)
                    .unwrap()
                    .as_non_fungible()
                    .take_non_fungibles(&ids)
                    .into(),
            };
            let receiver: Global<AnyComponent> = Global::from(receiver_address);
            receiver.call_raw::<()>("put_tokens", scrypto_args!(payment));
        }

        pub fn airdrop_staked_tokens(
            &mut self,
            claimants: IndexMap<Global<Account>, Decimal>,
            address: ResourceAddress,
            lock_duration: i64,
        ) {
            assert!(
                claimants.len() < 21,
                "Too many accounts to airdrop to! Try at most 20."
            );
            let mut to_airdrop_nfts: Option<Bucket> = None;
            let mut airdrop_map: IndexMap<Global<Account>, ResourceSpecifier> = IndexMap::new();

            for (receiver, amount) in claimants {
                let payment: Bucket = self
                    .vaults
                    .get_mut(&address)
                    .unwrap()
                    .as_fungible()
                    .take(amount)
                    .into();

                let staking_id: Bucket = self.staking.stake(payment, None).unwrap();

                if lock_duration > 0 {
                    let staking_proof: NonFungibleProof =
                        staking_id.as_non_fungible().create_proof_of_all();
                    let locking_reward: FungibleBucket =
                        self.staking
                            .lock_stake(address, staking_proof, lock_duration);
                    self.put_tokens(locking_reward.into());
                }
                let mut ids: IndexSet<NonFungibleLocalId> = IndexSet::new();
                ids.insert(staking_id.as_non_fungible().non_fungible_local_id());
                airdrop_map.insert(receiver, ResourceSpecifier::NonFungible(ids));

                match &mut to_airdrop_nfts {
                    Some(bucket) => bucket.put(staking_id),
                    None => to_airdrop_nfts = Some(staking_id),
                }
            }
            if let Some(to_airdrop_nfts) = to_airdrop_nfts {
                self.payment_locker
                    .airdrop(airdrop_map, to_airdrop_nfts, true);
            }
        }

        pub fn airdrop_tokens(
            &mut self,
            claimants: IndexMap<Global<Account>, ResourceSpecifier>,
            address: ResourceAddress,
        ) {
            assert!(
                claimants.len() < 31,
                "Too many accounts to airdrop to! Try at most 30."
            );
            let mut to_airdrop_tokens: Option<Bucket> = None;

            for (_receiver, specifier) in &claimants {
                match specifier {
                    ResourceSpecifier::Fungible(amount) => {
                        let payment: Bucket = self
                            .vaults
                            .get_mut(&address)
                            .unwrap()
                            .as_fungible()
                            .take(*amount)
                            .into();
                        match &mut to_airdrop_tokens {
                            Some(bucket) => bucket.put(payment),
                            None => to_airdrop_tokens = Some(payment),
                        }
                    }
                    ResourceSpecifier::NonFungible(ids) => {
                        let payment: Bucket = self
                            .vaults
                            .get_mut(&address)
                            .unwrap()
                            .as_non_fungible()
                            .take_non_fungibles(&ids)
                            .into();
                        match &mut to_airdrop_tokens {
                            Some(bucket) => bucket.put(payment),
                            None => to_airdrop_tokens = Some(payment),
                        }
                    }
                }
            }
            if let Some(to_airdrop_tokens) = to_airdrop_tokens {
                self.payment_locker
                    .airdrop(claimants, to_airdrop_tokens, true);
            }
        }

        pub fn employ(&mut self, job: Job) {
            self.employees.insert(
                job.employee,
                (job, Clock::current_time_rounded_to_minutes()),
            );
        }

        pub fn send_salary_to_employee(&mut self, employee: Global<Account>) {
            let mut employee_entry = self.employees.get_mut(&employee).unwrap();

            let last_payment: Instant = employee_entry.1;
            let job_duration: i64 = employee_entry.0.duration;
            let job_salary_token: ResourceAddress = employee_entry.0.salary_token;
            let job_salary: Decimal = employee_entry.0.salary;

            let periods_worked: Decimal = ((Clock::current_time_rounded_to_minutes()
                .seconds_since_unix_epoch
                - last_payment.seconds_since_unix_epoch)
                / (Decimal::from(job_duration) * dec!(86400)))
            .checked_floor()
            .unwrap();
            let whole_periods_worked: i64 =
                i64::try_from(periods_worked.0 / Decimal::ONE.0).unwrap();

            let payment: Bucket = self
                .vaults
                .get_mut(&job_salary_token)
                .unwrap()
                .as_fungible()
                .take(job_salary * periods_worked)
                .into();
            self.payment_locker.store(employee, payment, true);

            employee_entry.1 = last_payment
                .add_days(whole_periods_worked * job_duration)
                .unwrap();
        }

        pub fn fire(&mut self, employee: Global<Account>, salary_modifier: Option<Decimal>) {
            self.send_salary_to_employee(employee);
            let removed_job = self.employees.remove(&employee).unwrap();
            let payment: Bucket = self
                .vaults
                .get_mut(&removed_job.0.salary_token)
                .unwrap()
                .as_fungible()
                .take(removed_job.0.salary * salary_modifier.unwrap_or(dec!(1)))
                .into();

            self.payment_locker
                .store(removed_job.0.employee, payment, true);
        }

        pub fn set_parameters(
            &mut self,
            fee: Decimal,
            proposal_duration: i64,
            quorum: Decimal,
            approval_threshold: Decimal,
        ) {
            self.parameters.fee = fee;
            self.parameters.proposal_duration = proposal_duration;
            self.parameters.quorum = quorum;
            self.parameters.approval_threshold = approval_threshold;
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
            title: String,
            description: String,
            component: ComponentAddress,
            badge: ResourceAddress,
            method: String,
            args: ScryptoValue,
            return_bucket: bool,
            mut payment: Bucket,
        ) -> (Bucket, Bucket) {
            assert!(
                payment.resource_address() == self.mother_token_address
                    && payment.amount() > self.parameters.fee,
                "Invalid payment, must be more than the fee and correct token."
            );

            self.proposal_fee_vault
                .put(payment.take(self.parameters.fee));

            let first_step = ProposalStep {
                component,
                badge,
                method,
                args,
                return_bucket,
            };

            let proposal = Proposal {
                title,
                description,
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
            return_bucket: bool,
        ) {
            let receipt_proof = proposal_receipt_proof.check_with_message(
                self.proposal_receipt_manager.address(),
                "Invalid proposal receipt supplied!",
            );

            let receipt = receipt_proof.non_fungible::<ProposalReceipt>().data();
            assert!(
                receipt.status == ProposalStatus::Building,
                "Proposal is not being built!"
            );

            let proposal_id: u64 = receipt.proposal_id;
            let mut proposal = self.incomplete_proposals.get_mut(&proposal_id).unwrap();

            let step = ProposalStep {
                component,
                badge,
                method,
                args,
                return_bucket,
            };

            proposal.steps.push(step);
        }

        pub fn submit_proposal(&mut self, proposal_receipt_proof: NonFungibleProof) {
            let receipt_proof = proposal_receipt_proof.check_with_message(
                self.proposal_receipt_manager.address(),
                "Invalid proposal receipt supplied!",
            );

            let receipt = receipt_proof.non_fungible::<ProposalReceipt>().data();
            assert!(
                receipt.status == ProposalStatus::Building,
                "Proposal is not being built!"
            );

            let proposal_id: u64 = receipt.proposal_id;
            let mut proposal = self.incomplete_proposals.remove(&proposal_id).unwrap();

            proposal.status = ProposalStatus::Ongoing;
            proposal.deadline = Clock::current_time_rounded_to_minutes()
                .add_minutes(self.parameters.proposal_duration * 24 * 60)
                .unwrap();

            self.proposal_receipt_manager.update_non_fungible_data(
                &NonFungibleLocalId::integer(proposal_id),
                "status",
                proposal.status,
            );

            self.ongoing_proposals.insert(proposal_id, proposal);
        }

        pub fn vote_on_proposal(
            &mut self,
            proposal_id: u64,
            for_against: bool,
            voting_id_proof: NonFungibleProof,
        ) {
            let mut proposal = self.ongoing_proposals.get_mut(&proposal_id).unwrap();

            let id_proof = voting_id_proof
                .check_with_message(self.voting_id_address, "Invalid staking ID supplied!");
            let id: NonFungibleLocalId = id_proof.as_non_fungible().non_fungible_local_id();

            assert!(
                !Clock::current_time_is_at_or_after(proposal.deadline, TimePrecision::Minute),
                "Voting period has passed!"
            );
            assert!(
                proposal.votes.get(&id).is_none(),
                "Already voted on this proposal!"
            );

            let vote_power: Decimal = self
                .vaults
                .get_mut(&self.controller_badge_address)
                .unwrap()
                .as_fungible()
                .authorize_with_amount(dec!("0.75"), || {
                    self.staking
                        .vote(self.mother_token_address, proposal.deadline, id.clone())
                });

            if for_against {
                proposal.votes.insert(id.clone(), vote_power);
                proposal.votes_for += vote_power;
            } else {
                proposal.votes.insert(id.clone(), dec!("-1") * vote_power);
                proposal.votes_against += vote_power;
            }
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
            } else if proposal.votes_for > self.parameters.approval_threshold * total_votes {
                proposal.status = ProposalStatus::Accepted;
                self.accepted_proposals.insert(proposal_id, proposal);
            } else {
                proposal.status = ProposalStatus::Rejected;
                self.rejected_proposals.insert(proposal_id, proposal);
                decision = ProposalStatus::Rejected;
            }
            self.proposal_receipt_manager.update_non_fungible_data(
                &NonFungibleLocalId::integer(proposal_id),
                "status",
                decision,
            );

            if decision == ProposalStatus::Rejected {
                let fee_tokens: Bucket = self.proposal_fee_vault.take(fee_paid);
                self.put_tokens(fee_tokens);
            }
        }

        pub fn execute_proposal_step(&mut self, proposal_id: u64, steps_to_execute: i64) {
            let mut proposal = self.accepted_proposals.remove(&proposal_id).unwrap();

            for _ in 0..steps_to_execute {
                let step: &ProposalStep = &proposal.steps[proposal.next_index as usize];
                let component: Global<AnyComponent> = Global::from(step.component);
                let badge_vault: FungibleVault =
                    self.vaults.get_mut(&step.badge).unwrap().as_fungible();

                if step.return_bucket {
                    let bucket: Bucket = badge_vault.authorize_with_amount(dec!("0.75"), || {
                        component.call::<ScryptoValue, Bucket>(&step.method, &step.args)
                    });
                    self.put_tokens(bucket);
                } else {
                    badge_vault.authorize_with_amount(dec!("0.75"), || {
                        component.call::<ScryptoValue, ()>(&step.method, &step.args)
                    });
                }

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

            self.vaults
                .get_mut(&self.mother_token_address)
                .unwrap()
                .take((passed_minutes * self.daily_update_reward) / (dec!(24) * dec!(60)))
        }

        pub fn set_proxy_component(&mut self, proxy_component: ComponentAddress) {
            self.proxy = Global::from(proxy_component);
        }

        pub fn set_staking_component(
            &mut self,
            proxy_component: ComponentAddress,
            new_voting_id_address: ResourceAddress,
        ) {
            self.staking = proxy_component.into();
            self.voting_id_address = new_voting_id_address;
        }

        pub fn set_update_reward(&mut self, reward: Decimal) {
            self.daily_update_reward = reward;
        }
    }
}
