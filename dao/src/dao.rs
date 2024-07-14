//! # DAO Blueprint
//! 
//! The DAO blueprint is the main component of the DAO, holding all the information about the DAO, its employees, and its announcements.
//! It can be used to hire / fire employees. Airdrop (staked) tokens, send tokens, post / remove announcements, and some more.

use crate::bootstrap::bootstrap::*;
use crate::governance::governance::*;
use crate::staking::staking::*;
use scrypto::prelude::*;

/// Job structure, holding all information about a job in the DAO component.
#[derive(ScryptoSbor)]
pub struct Job {
    pub employee: Global<Account>,
    pub salary: Decimal,
    pub salary_token: ResourceAddress,
    pub duration: i64,
    pub recurring: bool,
    pub title: String,
    pub description: String,
}

#[blueprint]
mod dao {
    enable_method_auth! {
        methods {
            put_tokens => PUBLIC;
            send_tokens => restrict_to: [OWNER];
            take_tokens => restrict_to: [OWNER];
            employ => restrict_to: [OWNER];
            fire => restrict_to: [OWNER];
            airdrop_tokens => restrict_to: [OWNER];
            airdrop_staked_tokens => restrict_to: [OWNER];
            post_announcement => restrict_to: [OWNER];
            remove_announcement => restrict_to: [OWNER];
            set_update_reward => restrict_to: [OWNER];
            add_rewarded_call => restrict_to: [OWNER];
            remove_rewarded_calls => restrict_to: [OWNER];
            set_staking_component => restrict_to: [OWNER];
            send_salary_to_employee => PUBLIC;
            rewarded_update => PUBLIC;
            finish_bootstrap => PUBLIC;
            get_token_amount => PUBLIC;
        }
    }

    struct Dao {
        /// The staking component of the DAO.
        pub staking: Global<Staking>,
        /// The bootstrap component of the DAO. Used for the initial bootstrapping of liquidity.
        pub bootstrap: Global<LinearBootstrapPool>,
        /// The mother token of the DAO, used to govern it.
        pub mother_token_address: ResourceAddress,
        /// The vaults of the DAO, storing all fungible and non-fungible tokens.
        pub vaults: KeyValueStore<ResourceAddress, Vault>,
        /// Text announcements of the DAO.
        pub text_announcements: KeyValueStore<u64, String>,
        /// Counter for the text announcements.
        pub text_announcement_counter: u64,
        /// Last time the staking component was updated.
        pub last_update: Instant,
        /// Reward for updating the staking component.
        pub daily_update_reward: Decimal,
        /// Method calls that are rewarded.
        pub rewarded_calls: HashMap<ComponentAddress, Vec<String>>,
        /// Address of the controller badge.
        pub controller_badge_address: ResourceAddress,
        /// AccountLocker used by the DAO to pay people.
        pub payment_locker: Global<AccountLocker>,
        /// Employees of the DAO and their jobs.
        pub employees: KeyValueStore<Global<Account>, (Job, Instant)>,
        /// Governance component of the DAO.
        pub governance: Global<Governance>,
    }

    impl Dao {
        /// Instantiates a new DAO component.
        /// 
        /// # Input
        /// - `founder_allocation`: Percentage of the total supply to allocate to the founder.
        /// - `bootstrap_allocation`: Percentage of the total supply to allocate to the bootstrap pool.
        /// - `staking_allocation`: Percentage of the total supply to allocate to the staking pool.
        /// - `controller_badge`: Controller badge of the DAO.
        /// - `rewarded_calls`: Method calls that are rewarded.
        /// - `protocol_name`: Name of the protocol / DAO.
        /// - `protocol_token_supply`: Total supply of the DAO governance token.
        /// - `protocol_token_symbol`: Symbol of the DAO governance token.
        /// - `protocol_token_icon_url`: Icon URL of the protocol token.
        /// - `proposal_receipt_icon_url`: Icon URL of the proposal receipt.
        /// - `bootstrap_resource1`: Resource for the bootstrap pool.
        /// 
        /// # Output
        /// - The DAO component
        /// - the founder allocation bucket
        /// - a bucket that can't be dropped but will be empty
        /// - the bootstrap badge bucket used to reclaim initial bootstrap funds.
        /// 
        /// # Logic
        /// - Instantiate an AccountLocker
        /// - Mint DAO governance tokens (referred to as mother tokens)
        /// - Create the LinearBootstrapPool for the initial bootstrap
        /// - Create the Staking component
        /// - Instantiate the Governance component
        /// - Create the vaults for the mother tokens and store them
        /// - Store the rewarded methods
        /// - Instantiate the DAO component
        pub fn instantiate_dao(
            founder_allocation: Decimal,
            bootstrap_allocation: Decimal,
            staking_allocation: Decimal,
            mut controller_badge: Bucket,
            rewarded_calls: Option<(ComponentAddress, Vec<String>)>,
            protocol_name: String,
            protocol_token_supply: Decimal,
            protocol_token_symbol: String,
            protocol_token_icon_url: Url,
            proposal_receipt_icon_url: Url,
            bootstrap_resource1: Bucket,
            oci_dapp_definition: ComponentAddress,
        ) -> (Global<Dao>, Bucket, Option<Bucket>, Bucket) {
            let controller_badge_address: ResourceAddress = controller_badge.resource_address();

            let (address_reservation, component_address) =
                Runtime::allocate_component_address(Dao::blueprint_id());

            let payment_locker = Blueprint::<AccountLocker>::instantiate(
                OwnerRole::Fixed(rule!(require_amount(
                    dec!("0.75"),
                    controller_badge.resource_address()
                ))),
                rule!(
                    require_amount(dec!("0.75"), controller_badge.resource_address())
                        || require(global_caller(component_address))
                ),
                rule!(
                    require_amount(dec!("0.75"), controller_badge.resource_address())
                        || require(global_caller(component_address))
                ),
                rule!(
                    require_amount(dec!("0.75"), controller_badge.resource_address())
                        || require(global_caller(component_address))
                ),
                rule!(
                    require_amount(dec!("0.75"), controller_badge.resource_address())
                        || require(global_caller(component_address))
                ),
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
            let founder_allocation_amount: Decimal =
                founder_allocation * mother_token_bucket.amount();
            let staking_allocation_amount: Decimal =
                staking_allocation * mother_token_bucket.amount();
            let bootstrap_allocation_amount: Decimal =
                bootstrap_allocation * mother_token_bucket.amount();

            let (bootstrap, non_bucket, bootstrap_badge): (
                Global<LinearBootstrapPool>,
                Option<Bucket>,
                Bucket,
            ) = LinearBootstrapPool::new(
                bootstrap_resource1,
                mother_token_bucket.take(bootstrap_allocation_amount),
                dec!("0.99"),
                dec!("0.01"),
                dec!("0.5"),
                dec!("0.5"),
                dec!("0.002"),
                7,
                oci_dapp_definition,
            );

            let (staking, voting_id_address, pool_token_address): (Global<Staking>, ResourceAddress, ResourceAddress) = Staking::new(
                controller_badge.resource_address(),
                mother_token_bucket
                    .take(staking_allocation_amount)
                    .as_fungible(),
                1,
                protocol_name.clone(),
                protocol_token_symbol.clone(),
                31,
            );

            let vaults: KeyValueStore<ResourceAddress, Vault> = KeyValueStore::new();

            let founder_allocation_bucket: Bucket =
                mother_token_bucket.take(founder_allocation_amount);

            vaults.insert(
                mother_token_address,
                Vault::with_bucket(mother_token_bucket),
            );

            vaults.insert(
                controller_badge_address,
                Vault::with_bucket(controller_badge.take(1)),
            );

            let governance: Global<Governance> = Governance::instantiate_governance(
                controller_badge,
                protocol_name,
                protocol_token_symbol,
                proposal_receipt_icon_url,
                staking,
                mother_token_address,
                pool_token_address,
                voting_id_address,
            );

            let mut rewarded_calls_map: HashMap<ComponentAddress, Vec<String>> = HashMap::new();

            if let Some((component, method)) = rewarded_calls {
                rewarded_calls_map.insert(component, method);
            }

            let dao = Self {
                payment_locker,
                staking,
                bootstrap,
                mother_token_address,
                vaults,
                text_announcements: KeyValueStore::new(),
                text_announcement_counter: 0,
                last_update: Clock::current_time_rounded_to_minutes(),
                daily_update_reward: dec!(10000),
                rewarded_calls: rewarded_calls_map,
                controller_badge_address,
                employees: KeyValueStore::new(),
                governance,
            }
            .instantiate()
            .prepare_to_globalize(OwnerRole::Fixed(rule!(require(controller_badge_address))))
            .with_address(address_reservation)
            .globalize();

            (dao, founder_allocation_bucket, non_bucket, bootstrap_badge)
        }

        /// Finishes the bootstrap and stores the resulting LP-tokens to the DAO treasury
        pub fn finish_bootstrap(&mut self) {
            let tokens: Bucket = self.vaults.get_mut(&self.controller_badge_address).unwrap().as_fungible().authorize_with_amount(dec!(1), || self.bootstrap.finish_bootstrap());
            self.put_tokens(tokens)
                
        }

        /// Puts tokens into the DAO treasury
        /// 
        /// # Input
        /// - `tokens`: Tokens to put into the treasury
        /// 
        /// # Output
        /// - None
        /// 
        /// # Logic
        /// - If the resource address of the tokens is already in the vaults, put the tokens into the vault
        /// - Otherwise, create a new vault with the tokens and store it
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

        /// Sends tokens from the DAO treasury to a receiver
        /// 
        /// # Input
        /// - `address`: Address of the tokens to send
        /// - `tokens`: Tokens to send
        /// - `receiver_address`: Component address to send tokens to
        /// 
        /// # Output
        /// - None
        /// 
        /// # Logic
        /// - Take the tokens from the vault
        /// - Send the tokens to the receiver using the `put_tokens` method of the receiver component
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

        /// Takes tokens from the DAO treasury
        ///
        /// # Input
        /// - `address`: Address of the tokens to take
        /// - `tokens`: Tokens to take
        /// 
        /// # Output
        /// - The tokens taken
        ///
        /// # Logic
        /// - Take the tokens from the vault
        /// - Return the tokens taken
        pub fn take_tokens(
            &mut self,
            address: ResourceAddress,
            tokens: ResourceSpecifier,
        ) -> Bucket {
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
            payment
        }

        /// Staking tokens to receive a Staking ID through the Staking component, and then airdropping them using the Payment Locker
        /// 
        /// # Input
        /// - `claimants`: Claimants and the amount of tokens to airdrop to them
        /// - `address`: Address of the tokens to airdrop
        /// - `lock_duration`: Duration to lock the tokens for
        /// 
        /// # Output
        /// - None
        /// 
        /// # Logic
        /// - Assert that there are less than 21 claimants as airdropping too many at a time fails
        /// - Create a bucket to store the NFTs to airdrop
        /// - Create a map of claimants and their NFTs
        /// - For each claimant, stake the tokens, lock them if necessary, store the NFTs in the created bucket, and add the claimant to the map
        /// - Airdrop the NFTs using the map of claimants and bucket, through the Payment Locker
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

        /// Airdropping tokens through the Payment Locker
        /// 
        /// # Input
        /// - `claimants`: Claimants and amount/id of tokens to airdrop to them
        /// - `address`: Address of the tokens to airdrop
        /// 
        /// # Output
        /// - None
        /// 
        /// # Logic
        /// - Assert that there are less than 31 claimants as airdropping too many at a time fails
        /// - Create a bucket to store the tokens to airdrop
        /// - For each claimant take their to be airdropped tokens from the vault and put them in the bucket
        /// - Airdrop the tokens using the map of claimants and bucket, through the Payment Locker
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

        /// Employ a new employee
        /// 
        /// # Input
        /// - `job`: Job to employ the employee for
        /// 
        /// # Output
        /// - None
        /// 
        /// # Logic
        /// - Insert the employee and their job into the employees KVS
        pub fn employ(&mut self, job: Job) {
            self.employees.insert(
                job.employee,
                (job, Clock::current_time_rounded_to_minutes()),
            );
        }

        /// Send salary to an employee
        /// 
        /// # Input
        /// - `employee`: Employee to send the salary to
        /// 
        /// # Output
        /// - None
        /// 
        /// # Logic
        /// - Get the employee from the employees KVS
        /// - Calculate the periods worked by the employee
        /// - Take the salary from the vault
        /// - Trying to airdrop the salary to the employee, but storing it in the Payment Locker if it fails
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

        /// Fire an employee
        /// 
        /// # Input
        /// - `employee`: Employee to fire
        /// - `salary_modifier`: Modifier for the firing 'bonus'
        /// 
        /// # Output
        /// - None
        /// 
        /// # Logic
        /// - Send unclaimed salaries to employee
        /// - Remove the employee from the employees KVS
        /// - Take one more salary from the vault, multiplied by the salary_modifier
        /// - Send this final payment to the employee through the Payment Locker
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

        /// Post an announcement to the DAO
        pub fn post_announcement(&mut self, announcement: String) {
            self.text_announcements
                .insert(self.text_announcement_counter, announcement);
            self.text_announcement_counter += 1;
        }

        /// Remove an announcement from the DAO
        pub fn remove_announcement(&mut self, announcement_id: u64) {
            self.text_announcements.remove(&announcement_id);
        }

        /// Call the rewarded methods
        /// 
        /// # Input
        /// - None
        /// 
        /// # Output
        /// - The amount of tokens rewarded
        /// 
        /// # Logic
        /// - Calculate the time passed since the last update
        /// - Call all rewarded methods
        /// - Update the staking component (a standard rewarded method)
        pub fn rewarded_update(&mut self) -> Bucket {
            let passed_minutes: Decimal = (Clock::current_time_rounded_to_minutes()
                .seconds_since_unix_epoch
                - self.last_update.seconds_since_unix_epoch)
                / dec!(60);

            for (component_address, methods) in self.rewarded_calls.iter() {
                let component: Global<AnyComponent> = Global::from(component_address.clone());
                for method in methods {
                    component.call_raw::<()>(method, scrypto_args!());
                }
            }
            self.staking.update_period();
            self.last_update = Clock::current_time_rounded_to_minutes();

            self.vaults
                .get_mut(&self.mother_token_address)
                .unwrap()
                .take((passed_minutes * self.daily_update_reward) / (dec!(24) * dec!(60)))
        }

        /// Add a rewarded method call
        pub fn add_rewarded_call(&mut self, component: ComponentAddress, methods: Vec<String>) {
            self.rewarded_calls.insert(component, methods);
        }

        /// Remove a rewarded method call
        pub fn remove_rewarded_calls(&mut self, component: ComponentAddress) {
            self.rewarded_calls.remove(&component);
        }

        /// Set the staking component
        pub fn set_staking_component(
            &mut self,
            staking_component: ComponentAddress,
        ) {
            self.staking = staking_component.into();
        }

        /// Set the reward for calling the rewarded methods
        pub fn set_update_reward(&mut self, reward: Decimal) {
            self.daily_update_reward = reward;
        }

        /// Get the amount of tokens in possession of the DAO
        pub fn get_token_amount(&self, address: ResourceAddress) -> Decimal {
            self.vaults
                .get(&address)
                .unwrap()
                .as_fungible()
                .amount()
        }
    }
}
