#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::string_lit_as_bytes)]

//! A pallet to implememt the simplified Casper FFG consensus.
//! For example, the slash not implemented.
//! Just use the pallet to show how to introduce a new consensus for substrate.
//! Reference link: https://eips.ethereum.org/EIPS/eip-1011

use frame_support::{decl_event, decl_module, decl_storage, decl_error, dispatch, Parameter,
	traits::{Currency, Get, LockableCurrency, ReservableCurrency},
	weights::Weight, dispatch::DispatchResult,};
use frame_system::{self as system, ensure_signed};
use codec::{Codec, Decode, Encode};
use sp_runtime::traits::{AtLeast32Bit, MaybeSerialize, Member, One, Saturating, Zero, NumberFor};

use sp_std::collections::btree_set::BTreeSet;

#[cfg(test)]
mod tests;

#[derive(Encode, Decode, Clone, Copy, PartialEq, Eq)]
pub struct Validator<AccountId, Dynasty, BalanceOf> {
	pub deposit: BalanceOf,
	pub start_dynasty: Dynasty,
	pub end_dynasty: Dynasty,
	pub address: AccountId,
	pub withdraw_address: AccountId,
}

#[derive(Encode, Decode, Clone, PartialEq, Eq, Default)]
pub struct CheckPoints<BalanceOf, AccountId> {
	cur_dyn_deposits: BalanceOf,
	prev_dyn_deposits: BalanceOf,

	cur_dyn_votes: Vec<BalanceOf>,
	prev_dyn_votes: Vec<BalanceOf>,

	/// epoch is index of vector, 
	vote_account_set: Vec<BTreeSet<AccountId>>,
	is_justified: bool,
	is_finalized: bool,
}

type BalanceOf<T> = <<T as Trait>::Currency as Currency<<T as system::Trait>::AccountId>>::Balance;

/// The module's configuration trait.
pub trait Trait: system::Trait {
	/// The overarching event type.
	type Event: From<Event> + Into<<Self as system::Trait>::Event>;

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
		+ Into<u128>;

	/// Percentage of reward each epoch
	type RewardFactor: Get<u32>;

	/// Epoch Length
	type EpochLength: Get<u32>;

	/// WithDraw Delay in u32
	type WithdrawDelay: Get<u32>;

	/// type LogoutDelay: Get<u8>;
	type LogoutDelay: Get<u32>;

	type MinDeposit: Get<BalanceOf<Self>>;

}

decl_error! {
    pub enum Error for Module<T: Trait> {
        First,
    }
}

decl_storage! {
	trait Store for Module<T: Trait> as Casper {
		/// Simple variable
		pub NextValidatorId get(fn next_validator_id): T::ValidatorId;
		pub CurrentEpoch get(fn current_epoch): T::Epoch;
		pub CurrentDynasty get(fn current_dynasty): T::Dynasty;
		pub ExpectedSourceEpoch get(fn expected_source_epoch): T::Epoch;

		pub MainHashJustified get(fn main_hash_justified): bool;
		pub LastFinalizedEpoch get(fn last_finalized_epoch): T::Epoch;
		pub LastJustifiedEpoch get(fn last_justified_epoch): T::Epoch;

		pub CurrentDynastyDeposits get(fn current_dynasty_deposits): BalanceOf<T>;
		pub PrevDynastyDeposits get(fn prev_dynasty_deposits): BalanceOf<T>;

		/// Map variable
		pub ValidatorById get(fn validator_by_id): map hasher(twox_64_concat) T::ValidatorId => Option<Validator<T::AccountId, T::Dynasty, BalanceOf<T>>>;
		pub ValidatorIdByAccount get(fn validator_id_by_account): map hasher(twox_64_concat) T::AccountId => Option<T::ValidatorId>;
		pub CheckPointHash get(fn check_point_hash): map hasher(twox_64_concat) T::Dynasty => T::Hash;

		pub DynastyBalanceDelta get(fn dynasty_balance_delta): map hasher(twox_64_concat) T::Dynasty => BalanceOf<T>;
		pub TotalCurrentDynastyDeposit get(fn total_cur_dyn_deposits): BalanceOf<T>;
		pub TotalPreDynastyDeposit get(fn total_pre_dyn_deposits): BalanceOf<T>;
		pub DynastyStartEpoch get(fn dynasty_start_epoch): map hasher(twox_64_concat) T::Dynasty => T::Epoch;
		pub DynastyInEpoch get(fn dynasty_in_epoch): map hasher(twox_64_concat) T::Dynasty => T::Epoch;
		pub CheckPointsByEpoch get(fn check_points_by_epoch): map hasher(twox_64_concat) T::Epoch => CheckPoints<BalanceOf<T>, T::AccountId>;
		/// Map with struct.


		Thing1 get(fn thing1): u32;
		Thing2 get(fn thing2): u32;
	}
}

// The module's dispatchable functions.
decl_module! {
	/// The module declaration.
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		fn deposit_event() = default;
		/// deposit to become a validator
		#[weight = 10_000]
		pub fn deposit(origin, amount: BalanceOf<T>, end_dynasty: T::Dynasty, withdraw_address: T::AccountId) -> dispatch::DispatchResult {
			// Check that its a valid signature
			let who = ensure_signed(origin)?;

			// Reserve fee for thread and post
			<T as Trait>::Currency::reserve(&who, amount)?;

			if Self::validator_id_by_account(withdraw_address.clone()).is_some() {
				/// withdraw address already registered before.
				return Err(Error::<T>::First.into());
			};

			if amount < T::MinDeposit::get() {
				/// Reserved balance must more than minimum deposit
				return Err(Error::<T>::First.into());
			};

			<T as Trait>::Currency::reserve(&who, amount)?;

			let start_dynasty = <CurrentDynasty<T>>::get() + One::one() + One::one();

			let validator = Validator {
				deposit: amount,
				start_dynasty: start_dynasty,
				end_dynasty: end_dynasty,
				address: who,
				withdraw_address: withdraw_address.clone(),
			};

			/// Set deposit delta for start dynasty 
			<DynastyBalanceDelta<T>>::mutate(start_dynasty, |value| *value += amount);

			/// Put new validator into validator map
			<ValidatorById<T>>::mutate(<NextValidatorId<T>>::get(), |value| *value = Some(validator));

			/// Set validator's withdraw address
			<ValidatorIdByAccount<T>>::mutate(withdraw_address.clone(), |value| *value = Some(<NextValidatorId<T>>::get()));
			
			/// Increment the next validator id
			<NextValidatorId<T>>::mutate(|value| *value += One::one());

			Ok(())
		}

		/// vote 
		#[weight = 10_000]
		pub fn vote(origin, validator_id: T::ValidatorId, target_hash: T::Hash, source_epoch: T::Epoch, target_epoch: T::Epoch) {

		}
		/// logout
		#[weight = 10_000]
		pub fn logout(origin, validator_id: T::ValidatorId,) {

		}

		/// On block Initialization
		fn on_initialize(now: T::BlockNumber) -> Weight {
			let zero_block_number: T::BlockNumber = 0.into();
			let epoch_length_as_block_number: T::BlockNumber = T::EpochLength::get().into();

			/// Init epoch if block number is times of epoch length.
			if now % epoch_length_as_block_number == zero_block_number {
				let epoch_number = now / epoch_length_as_block_number;
				Self::initialize_epoch(epoch_number.into());
			}

			0
		}

		/// Sets the first simple storage value
		#[weight = 10_000]
		pub fn set_thing_1(origin, val: u32) -> dispatch::DispatchResult {
			let _ = ensure_signed(origin)?;

			Thing1::put(val);

			Self::deposit_event(Event::ValueSet(1, val));
			Ok(())
		}

		/// Sets the second stored value
		#[weight = 10_000]
		pub fn set_thing_2(origin, val: u32) -> dispatch::DispatchResult {
			let _ = ensure_signed(origin)?;

			Thing2::put(val);

			Self::deposit_event(Event::ValueSet(2, val));
			Ok(())
		}
	}
}

impl<T: Trait> Module<T> {
	pub fn get_sum() -> u32 {
		Thing1::get() + Thing2::get()
	}

	fn get_hash(block_number: T::BlockNumber) -> T::Hash {
		<system::Module<T>>::block_hash(block_number)
	}

	fn in_dynasty(validator_id: T::ValidatorId, dynasty: T::Dynasty) -> bool {
		true
	}

	fn votable(validator_id: T::ValidatorId, source_epoch: T::Epoch, target_epoch: T::Epoch,
		target_hash: T::Hash) -> bool {
		
		// 		target_hash: bytes32,
	// 		target_epoch: uint256,
	// 		source_epoch: uint256) -> bool:
	// 	# Check that this vote has not yet been made
	// 	already_voted: uint256 = bitwise_and(
	// 	self.checkpoints[target_epoch].vote_bitmap[validator_index / 256],
	// 	shift(convert(1, 'uint256'), convert(validator_index % 256, "int128"))
	// 	)
	// 	if already_voted:
	// 	return False
	// 	# Check that the vote's target epoch and hash are correct
	// 	if target_hash != self.recommended_target_hash():
	// 	return False
	// 	if target_epoch != self.current_epoch:
	// 	return False
	// 	# Check that the vote source points to a justified epoch
	// 	if not self.checkpoints[source_epoch].is_justified:
	// 	return False

	// 	# ensure validator can vote for the target_epoch
	// 	in_current_dynasty: bool = self.in_dynasty(validator_index, self.dynasty)
	// 	in_prev_dynasty: bool = self.in_dynasty(validator_index, self.dynasty - 1)
	//	return in_current_dynasty or in_prev_dynasty

			true
	}

	fn increment_dynasty() {
		// if checkpoints[epoch - 2].is_finalized {
		// 	Dynasty.mutate(|value| *value += 1);
		// 	self.total_prevdyn_deposits = self.total_curdyn_deposits;
        // 	self.total_curdyn_deposits += self.dynasty_wei_delta[self.dynasty];
        // 	self.dynasty_start_epoch[self.dynasty] = epoch;
		// }

		// T::dynasty_in_epoch[T::Epoch] = dynasty;

		// if T::MainHashJustified {
		// 	T::ExpectedSourceEpoch = T::CurrentEpoch - 1
		// }

		MainHashJustified::mutate(|value| *value = false);
	}

	fn instant_finalize() {
		MainHashJustified::mutate(|value| *value = true);
		<LastFinalizedEpoch<T>>::mutate(|value| *value = Self::current_epoch());
		<LastJustifiedEpoch<T>>::mutate(|value| *value = Self::current_epoch());
		<CheckPointsByEpoch<T>>::mutate(Self::current_epoch() - One::one(),
			|value| *value = CheckPoints {
				is_justified: true,
				is_finalized: true,
				..Default::default()
			})
	}

	fn deposit_exists() -> bool {
		Self::current_dynasty_deposits() > Zero::zero() && 
		Self::prev_dynasty_deposits() > Zero::zero()
	}

	pub fn initialize_epoch(epoch: T::Epoch) -> DispatchResult {
		/// New epoch must be equal current plus one
		if epoch != Self::current_epoch() + One::one() {
			Err(Error::<T>::First.into())
		} else {
			let new_checkpoint = CheckPoints {
				cur_dyn_deposits: Self::current_dynasty_deposits(),
				prev_dyn_deposits: Self::prev_dynasty_deposits(),
			
				/// balance for each dynasty
				cur_dyn_votes: vec![],
				prev_dyn_votes: vec![],
			
				/// epoch is index of vector, 
				vote_account_set: Vec::<BTreeSet<T::AccountId>>::new(),
				is_justified: false,
				is_finalized: false,
			};

			<CurrentEpoch<T>>::mutate(|value| *value = epoch);

			/// Before the first validator deposits, new epochs are finalized instantly.
			Self::instant_finalize();

			/// Store checkout point hash for each epoch
			let epoch_length_as_epoch: T::Epoch = T::EpochLength::get().into();
			<CheckPointHash<T>>::mutate(Self::current_dynasty(), |value| *value = Self::get_hash((epoch * epoch_length_as_epoch).into()));

			Ok(())
		}
		// checkpoints[epoch].cur_dyn_deposits = total_cur_dyn_deposits;
		// checkpoints[epoch].prev_dyn_deposits = total_cur_dyn_deposits;

		// Update current epoch
		// current_epoch = epoch;

	}

	//pub fn votable() {

	
	// }
}

decl_event!(
	pub enum Event {
		ValueSet(u32, u32),
	}
);
