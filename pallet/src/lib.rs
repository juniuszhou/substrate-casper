#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::string_lit_as_bytes)]

//! A pallet to implememt the simplified Casper FFG consensus.
//! For example, the slash not implemented, the reward interests is a fixed number,
//! 1 / 10000 for example. 
//! Just use the pallet to show how to introduce a new consensus for substrate.
//! Reference link: https://eips.ethereum.org/EIPS/eip-1011
//! Python Implementation: https://github.com/ethereum/casper/tree/master/casper/contracts

use frame_support::{decl_event, decl_module, decl_storage, decl_error, dispatch, Parameter, debug,
	traits::{Currency, Get, LockableCurrency, ReservableCurrency},
	weights::Weight,};
use frame_system::{self as system, ensure_signed};
use codec::{Codec, Decode, Encode};
use sp_runtime::traits::{AtLeast32Bit, MaybeSerialize, Member, One, Zero, CheckedDiv, Saturating};

use sp_std::collections::btree_set::BTreeSet;

#[cfg(test)]
mod tests;

/// Info for each validator
#[derive(Encode, Decode, Clone, Copy, PartialEq, Eq)]
pub struct Validator<AccountId, Dynasty, BalanceOf> {
	pub deposit: BalanceOf,
	pub reward: BalanceOf,
	pub start_dynasty: Dynasty,
	pub end_dynasty: Dynasty,
	pub address: AccountId,
	pub withdraw_address: AccountId,
}

/// Check point record for each dynasty
#[derive(Encode, Decode, Clone, Default)]
pub struct CheckPoints<BalanceOf, ValidatorId> where ValidatorId: Ord {
	cur_dyn_deposits: BalanceOf,
	prev_dyn_deposits: BalanceOf,
	vote_account_set: BTreeSet<ValidatorId>,
	is_justified: bool,
	is_finalized: bool,
}

type BalanceOf<T> = <<T as Trait>::Currency as Currency<<T as system::Trait>::AccountId>>::Balance;

/// The module's configuration trait.
pub trait Trait: system::Trait {
	type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;

	/// Currency type for this module.
	type Currency: ReservableCurrency<Self::AccountId>
	+ LockableCurrency<Self::AccountId, Moment = Self::BlockNumber>;

	type Dynasty: Parameter
		+ AtLeast32Bit
		+ Codec
		+ Default
		+ Copy
		+ MaybeSerialize
		+ PartialEq
		+ Member
		+ From<u32>
		+ Into<u32>;
	
	type Epoch: Parameter
		+ AtLeast32Bit
		+ Codec
		+ Default
		+ Copy
		+ MaybeSerialize
		+ PartialEq
		+ Member
		+ From<u32>
		+ Into<u32>
		+ From<Self::BlockNumber>
		+ Into<Self::BlockNumber>;

	type ValidatorId: Parameter
		+ AtLeast32Bit
		+ Codec
		+ Default
		+ Copy
		+ MaybeSerialize
		+ PartialEq
		+ Member
		+ From<u128>
		+ Into<u128>
		+ Eq
		+ Ord;

	/// Reward = Deposit * RewardFactor / RewardRateBase
	type RewardFactor: Get<u32>;
	type RewardRateBase: Get<u32>;

	/// Epoch Length
	type EpochLength: Get<u32>;

	/// WithDraw Delay in u32
	type WithdrawDelay: Get<u32>;

	/// type LogoutDelay: Get<u8>;
	type LogoutDelay: Get<u32>;

	/// Minimum deposit balance to be validator
	type MinDeposit: Get<BalanceOf<Self>>;
}

decl_error! {
    pub enum Error for Module<T: Trait> {
		EpochNotReachedForLogout,
		ValidatorNotRegistered,
		OnlyWithdrawAccountCanLogout,
		DynastyNotReachedForLogout,
		DynastyNotReachedForWithdraw,
		EpochNotReachedForWithdraw,
		ValidatorIdNotValid,
		DepositLessThanMinimum,
		WrongTargetEpoch,
		VoteSenderDifferWithRegistered,
		SenderVotedBefore,
		VoteOnWrongBlockHash,
		VoteNotInCorrectDynasty,
		VotedSourceEpochNotJustified,
    }
}

decl_storage! {
	trait Store for Module<T: Trait> as Casper {
		/// Simple variable
		pub NextValidatorId get(fn next_validator_id): T::ValidatorId;
		pub CurrentEpoch get(fn current_epoch): T::Epoch;
		pub CurrentDynasty get(fn current_dynasty): T::Dynasty;

		/// Epoch as source epoch for vote
		pub ExpectedSourceEpoch get(fn expected_source_epoch): T::Epoch;

		/// If the block hash of current epoch justified
		pub MainHashJustified get(fn main_hash_justified): bool;

		pub LastFinalizedEpoch get(fn last_finalized_epoch): T::Epoch;
		pub LastJustifiedEpoch get(fn last_justified_epoch): T::Epoch;
		pub TotalCurrentDynastyDeposit get(fn total_cur_dyn_deposits): BalanceOf<T>;
		pub TotalPreDynastyDeposit get(fn total_pre_dyn_deposits): BalanceOf<T>;

		pub RecommendedTargetHash get(fn recommended_target_hash): T::Hash;

		/// Map variable
		pub ValidatorById get(fn validator_by_id): map hasher(twox_64_concat) T::ValidatorId => Option<Validator<T::AccountId, T::Dynasty, BalanceOf<T>>>;
		
		/// Validator Id map to withdraw account
		pub ValidatorIdByAccount get(fn validator_id_by_account): map hasher(twox_64_concat) T::AccountId => T::ValidatorId;
		
		/// Hash for each dynasty to be finalized
		pub CheckPointHash get(fn check_point_hash): map hasher(twox_64_concat) T::Dynasty => T::Hash;

		/// Since balance not support the signed type, we use two maps to record the deposti delta from dynasty to dynasty
		/// New balance deposited in dynasty   DynastyBalanceDelta
		pub NewDepositInDynasty get(fn new_deposity_in_dynasty): map hasher(twox_64_concat) T::Dynasty => BalanceOf<T>;
		
		/// Deposited balance withdrawed in dynasty
		pub WithdrawedDepositInDynasty get(fn withdrawed_deposit_in_dynasty): map hasher(twox_64_concat) T::Dynasty => BalanceOf<T>;

		/// Start epoch for each dynasty
		pub DynastyStartEpoch get(fn dynasty_start_epoch): map hasher(twox_64_concat) T::Dynasty => T::Epoch;
		
		/// Map each epoch to dynasty
		pub DynastyInEpoch get(fn dynasty_in_epoch): map hasher(twox_64_concat) T::Epoch => T::Dynasty;

		/// CheckPoint information for each epoch
		pub CheckPointsByEpoch get(fn check_points_by_epoch): map hasher(twox_64_concat) T::Epoch => CheckPoints<BalanceOf<T>, T::ValidatorId>;
		
		/// Double Map
		pub CurrentDynastyVotes get(fn current_dynasty_votes): double_map hasher(twox_64_concat) T::Dynasty, hasher(twox_64_concat) T::Epoch => BalanceOf<T>;
		pub PreDynastyVotes get(fn pre_dynasty_votes): double_map hasher(twox_64_concat) T::Dynasty, hasher(twox_64_concat) T::Epoch => BalanceOf<T>;
	}
}

// The module's dispatchable functions.
decl_module! {
	/// The module declaration.
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		fn deposit_event() = default;

		/// Validator logout from casper contract
		#[weight = 10_000]
		pub fn logout(origin, validator_id: T::ValidatorId, epoch: T::Epoch) -> dispatch::DispatchResult {
			let who = ensure_signed(origin)?;

			// Check epoch for logout
			if <CurrentEpoch<T>>::get() < epoch {
				return Err(Error::<T>::EpochNotReachedForLogout.into());
			};

			if let Some(mut validator) = <ValidatorById<T>>::get(validator_id) {
				// Check sender must be withdraw address
				if validator.withdraw_address != who {
					return Err(Error::<T>::OnlyWithdrawAccountCanLogout.into());
				}

				// Check dynasty for logout
				if validator.end_dynasty < <CurrentDynasty<T>>::get() + T::LogoutDelay::get().into() {
					return Err(Error::<T>::DynastyNotReachedForLogout.into());
					
				} else {
					// Update dynasty then we can withdraw deposit later
					validator.end_dynasty = <CurrentDynasty<T>>::get() + T::LogoutDelay::get().into();
				}
			} else {
				return Err(Error::<T>::ValidatorNotRegistered.into());
			}

			Self::deposit_event(RawEvent::Logout(validator_id, epoch));

			Ok(())
		}

		/// Validator withdraw depostied currency
		#[weight = 10_000]
		pub fn withdraw(origin, validator_id: T::ValidatorId) -> dispatch::DispatchResult {
			// No verify if the sender from the address of validator, since the deposit and interests will 
			// go to withdraw account after end dynasty
			let _ = ensure_signed(origin)?;

			if let Some(validator) = <ValidatorById<T>>::get(validator_id) {
				// Can't withdraw until end dynasty 
				if <CurrentDynasty<T>>::get() > validator.end_dynasty {
					return Err(Error::<T>::DynastyNotReachedForWithdraw.into());
				} else {
					if <DynastyStartEpoch<T>>::get(validator.end_dynasty + One::one()) + T::WithdrawDelay::get().into() < <CurrentEpoch<T>>::get() {
						return Err(Error::<T>::EpochNotReachedForWithdraw.into());
					} else {
						// Unreserve the balance to withdraw account
						<T as Trait>::Currency::unreserve(&validator.withdraw_address, validator.deposit);

						// Put reward into withdraw account
						<T as Trait>::Currency::deposit_into_existing(&validator.withdraw_address, validator.reward)?;

						// Remove withdraw address from map
						<ValidatorIdByAccount<T>>::remove(validator.withdraw_address);

						// Remove the validator from list
						<ValidatorById<T>>::remove(validator_id);
					}
				}
			} else {
				return Err(Error::<T>::ValidatorNotRegistered.into());
			}

			Self::deposit_event(RawEvent::Withdraw(validator_id));
	
			Ok(())
		}

		/// deposit to become a validator
		#[weight = 10_000]
		pub fn deposit(origin, amount: BalanceOf<T>, end_dynasty: T::Dynasty, withdraw_address: T::AccountId) -> dispatch::DispatchResult {
			// Check that its a valid signature
			let who = ensure_signed(origin)?;

			// Check if withdraw address registered before
			if <ValidatorIdByAccount<T>>::get(&withdraw_address) == Zero::zero() {
				return Err(Error::<T>::ValidatorIdNotValid.into());
			};

			// Reserved balance must be more than minimum deposit
			if amount < T::MinDeposit::get() {
				return Err(Error::<T>::DepositLessThanMinimum.into());
			};

			// Reserve balance to be validator
			<T as Trait>::Currency::reserve(&who, amount)?;

			// Increment the next validator id before put validator into map
			// So we skip the validator id as 0 reserved for not registered
			<NextValidatorId<T>>::mutate(|value| *value += One::one());

			// Start dynasty set as two dynasty later according to spec
			let start_dynasty = <CurrentDynasty<T>>::get() + One::one() + One::one();

			let validator = Validator {
				deposit: amount,
				reward: Zero::zero(),
				start_dynasty: start_dynasty,
				end_dynasty: end_dynasty,
				address: who.clone(),
				withdraw_address: withdraw_address.clone(),
			};

			// Set deposit delta for start dynasty 
			<NewDepositInDynasty<T>>::mutate(start_dynasty, |value| *value += amount);

			// Put new validator into validator map
			<ValidatorById<T>>::mutate(<NextValidatorId<T>>::get(), |value| *value = Some(validator));

			// Set validator's withdraw address
			<ValidatorIdByAccount<T>>::mutate(withdraw_address.clone(), |value| *value = <NextValidatorId<T>>::get());
			
			Self::deposit_event(RawEvent::Deposit(who, withdraw_address, end_dynasty));

			Ok(())
		}

		/// Validator vote with target hash
		#[weight = 10_000]
		pub fn vote(origin, validator_id: T::ValidatorId, source_epoch: T::Epoch, target_epoch: T::Epoch, target_hash: T::Hash) -> 
			dispatch::DispatchResult {
			// Check that its a valid signature
			let who = ensure_signed(origin)?;

			// Check if vote is valid
			Self::ensure_votable(who.clone(), validator_id, source_epoch, target_epoch, target_hash)?;

			let mut new_current_dynasty_votes = <CurrentDynastyVotes<T>>::get(<CurrentDynasty<T>>::get(), source_epoch);
			let mut new_previous_dynasty_votes = <PreDynastyVotes<T>>::get(<CurrentDynasty<T>>::get(), source_epoch);

			
			if Self::in_dynasty(validator_id, <CurrentDynasty<T>>::get()) {
				if let Some(validator) = <ValidatorById<T>>::get(validator_id) {
					new_current_dynasty_votes += validator.deposit;
				};
			};

			if <CurrentDynasty<T>>::get() > Zero::zero() && Self::in_dynasty(validator_id, <CurrentDynasty<T>>::get() - One::one()) {
				if let Some(validator) = <ValidatorById<T>>::get(validator_id) {
					new_previous_dynasty_votes += validator.deposit;
				};
			};

			if <ExpectedSourceEpoch<T>>::get() == source_epoch {
				Self::proc_reward(validator_id);
			}

			if new_current_dynasty_votes.saturating_mul(3.into()) >= <TotalCurrentDynastyDeposit<T>>::get().saturating_mul(2.into()) &&
				new_previous_dynasty_votes.saturating_mul(3.into()) >= <TotalPreDynastyDeposit<T>>::get().saturating_mul(2.into()) {
				// Justify the epoch if enough vote agreed on the hash
					if !<CheckPointsByEpoch<T>>::get(target_epoch).is_justified {
					<CheckPointsByEpoch<T>>::mutate(target_epoch, |value| value.is_justified = true);
					<LastFinalizedEpoch<T>>::mutate(|value| *value = target_epoch);
					MainHashJustified::set(true);

					// Finalize the source epoch if two epochs justified consecutively
					if target_epoch == source_epoch + One::one() {
						<LastFinalizedEpoch<T>>::mutate(|value| *value = source_epoch);
						<CheckPointsByEpoch<T>>::mutate(source_epoch, |value| value.is_finalized = true);
					}
				}
			}

			<CheckPointsByEpoch<T>>::mutate(target_epoch, |value| value.vote_account_set.insert(validator_id));

			Self::deposit_event(RawEvent::Vote(who, validator_id, source_epoch, target_epoch, target_hash));

			Ok(())
		}

		/// On block Initialization
		fn on_initialize(now: T::BlockNumber) -> Weight {
			let one_block_number: T::BlockNumber = 1.into();
			let epoch_length_as_block_number: T::BlockNumber = T::EpochLength::get().into();

			// Init epoch if block number is times of epoch length.
			if now % epoch_length_as_block_number == one_block_number {
				let epoch_number = now / epoch_length_as_block_number;
				let block_number_as_epoch: T::Epoch = epoch_number.into();
				Self::initialize_epoch(block_number_as_epoch);
			}

			0
		}
	}
}

impl<T: Trait> Module<T> {

	pub fn proc_reward(validator_id: T::ValidatorId) {
		if let Some(mut validator) = <ValidatorById<T>>::get(validator_id) {
			// Compute the reward with fixed interests
			let reward = validator.deposit.checked_div(&<BalanceOf<T>>::from(T::RewardRateBase::get())).unwrap().saturating_mul(<BalanceOf<T>>::from(T::RewardFactor::get()));
			validator.reward += reward;

			let start_dynasty = validator.start_dynasty;
			let end_dynasty = validator.end_dynasty;
			let current_dynasty = <CurrentDynasty<T>>::get();
			
			// Update current dynasty deposit or past dynasty deposit.
			if start_dynasty <= current_dynasty && current_dynasty < end_dynasty {
				<TotalCurrentDynastyDeposit<T>>::mutate(|value| *value += reward);
			};

			if current_dynasty > Zero::zero() {
				let past_dynasty = current_dynasty - One::one();
				if start_dynasty <= past_dynasty && past_dynasty < end_dynasty {
					<TotalPreDynastyDeposit<T>>::mutate(|value| *value += reward);
				}
			};
	
			<WithdrawedDepositInDynasty<T>>::mutate(end_dynasty, |value| *value += validator.deposit);
		}
	}

	

	fn get_hash(block_number: T::BlockNumber) -> T::Hash {
		<system::Module<T>>::block_hash(block_number)
	}

	fn in_dynasty(validator_id: T::ValidatorId, dynasty: T::Dynasty) -> bool {
		let validator = Self::validator_by_id(validator_id);
		match validator {
			Some(data) => {
				data.start_dynasty <= dynasty && dynasty < data.end_dynasty
			},
			None => false,
		}
	}

	/// Check if a validator can vote with correct parameter source epoch, target epoch and target hash
	fn ensure_votable(sender: T::AccountId, validator_id: T::ValidatorId, source_epoch: T::Epoch, target_epoch: T::Epoch, 
		target_hash: T::Hash) -> dispatch::DispatchResult {
	
		// Only vote for current epoch is allowed
		if target_epoch != Self::current_epoch() {
			return Err(Error::<T>::WrongTargetEpoch.into());
		}

		// Sender must be the account who deposit to be validator
		if let Some(validator) = Self::validator_by_id(validator_id) {
			if validator.address != sender {
				return Err(Error::<T>::VoteSenderDifferWithRegistered.into());
			}
		} else {
			return Err(Error::<T>::ValidatorNotRegistered.into());
		}
		
		// Check if validator voted before
		if Self::check_points_by_epoch(target_epoch).vote_account_set.contains(&validator_id) {
			return Err(Error::<T>::SenderVotedBefore.into());
		}
		
		// Must vote on correct target hash
		if target_hash != Self::recommended_target_hash() {
			return Err(Error::<T>::VoteOnWrongBlockHash.into());
		}

		// Source epoch must be justified
		if Self::in_dynasty(validator_id, Self::current_dynasty()) || 
			Self::in_dynasty(validator_id, Self::current_dynasty() - One::one()) {
			if Self::check_points_by_epoch(source_epoch).is_justified {
				Ok(())
			} else {
				Err(Error::<T>::VotedSourceEpochNotJustified.into())
			}
		} else {
			 Err(Error::<T>::VoteNotInCorrectDynasty.into())
		}
	}

	fn increment_dynasty() {
		// Finalized check with two epoch before
		if <CurrentEpoch<T>>::get() > One::one() {
			
			let is_finalized = <CheckPointsByEpoch<T>>::get(<CurrentEpoch<T>>::get() - One::one() - One::one()).is_finalized;
	
			if is_finalized {
				// Increment the dynasty if it was finalized tow epoch ago
				<CurrentDynasty<T>>::mutate(|value| *value += One::one());
	
				// Rotate the deposit for previous and current dynasty
				<TotalPreDynastyDeposit<T>>::mutate(|value| *value = <TotalCurrentDynastyDeposit<T>>::get());
				<TotalCurrentDynastyDeposit<T>>::mutate(|value| *value = <NewDepositInDynasty<T>>::get(<CurrentDynasty<T>>::get()) -
				<WithdrawedDepositInDynasty<T>>::get(<CurrentDynasty<T>>::get()));
	
				// Map the dynasty to epoch
				<DynastyStartEpoch<T>>::mutate(<CurrentDynasty<T>>::get(), |value| *value = <CurrentEpoch<T>>::get());
			}
		}

		// Map epoch to dynasty
		<DynastyInEpoch<T>>::mutate(<CurrentEpoch<T>>::get(), |value| *value = <CurrentDynasty<T>>::get());

		// If main hash justified then update expected source epoch
		if MainHashJustified::get() {
			<ExpectedSourceEpoch<T>>::mutate(|value| *value = <CurrentEpoch<T>>::get() - One::one());
		}

		// Update 
		MainHashJustified::mutate(|value| *value = false);
	}

	/// Finalize last epoch
	fn instant_finalize() {
		if Self::current_epoch() < One::one() {
			return;
		}

		let last_epoch = Self::current_epoch() - One::one();
		MainHashJustified::mutate(|value| *value = true);

		<LastFinalizedEpoch<T>>::mutate(|value| *value = last_epoch);
		<LastJustifiedEpoch<T>>::mutate(|value| *value = last_epoch);

		// Finalize last epoch
		<CheckPointsByEpoch<T>>::mutate(last_epoch,
			|value| {
				value.is_justified = true;
				value.is_finalized = true;
			});

	}

	fn deposit_exists() -> bool {
		Self::total_cur_dyn_deposits() > Zero::zero() && 
		Self::total_pre_dyn_deposits() > Zero::zero()
	}

	/// Initialize a new epoch
	pub fn initialize_epoch(epoch: T::Epoch) {
		// New epoch must be equal current plus one
		if epoch == Self::current_epoch() + One::one() || epoch == Zero::zero() {
			// Insert new check point 
			<CheckPointsByEpoch<T>>::insert(epoch, CheckPoints {
				cur_dyn_deposits: Self::total_cur_dyn_deposits(),
				prev_dyn_deposits: Self::total_pre_dyn_deposits(),
				vote_account_set: BTreeSet::<T::ValidatorId>::new(),
				..Default::default()
			});

			// Update current epoch
			<CurrentEpoch<T>>::mutate(|value| *value = epoch);

			// If no deposit, new epoch is finalized instantly.
			if !Self::deposit_exists() {
				Self::instant_finalize();
			}

			// Store checkout point hash for each epoch
			let epoch_length_as_epoch: T::Epoch = T::EpochLength::get().into();
			<CheckPointHash<T>>::mutate(Self::current_dynasty(), 
				|value| *value = Self::get_hash((epoch * epoch_length_as_epoch).into()));
			
			// Try to increment dynasty
			Self::increment_dynasty();
 
			// Set recommended target hash as last epoch's hash
			if epoch != Zero::zero() {
				<RecommendedTargetHash<T>>::mutate(|value| *value = 
					Self::get_hash(((epoch - One::one()) * epoch_length_as_epoch).into()));
			}
		}
	}
}

decl_event!(
	pub enum Event<T> 
	where 
	<T as system::Trait>::AccountId,
	<T as system::Trait>::Hash,
	<T as Trait>::Dynasty,
	<T as Trait>::Epoch,
	<T as Trait>::ValidatorId,
	{
		Deposit(AccountId, AccountId, Dynasty),
		Vote(AccountId, ValidatorId, Epoch, Epoch, Hash),
		Logout(ValidatorId, Epoch),
		Withdraw(ValidatorId),
	}
);

