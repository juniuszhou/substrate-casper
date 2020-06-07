#![cfg_attr(not(feature = "std"), no_std)]

use sp_runtime::traits::Block as BlockT;

// Runtime API for casper pallet
sp_api::decl_runtime_apis! {

	pub trait CasperRuntimeApi<Epoch> where
		Epoch: codec::Codec, {
		fn get_highest_finalized_epoch() -> Epoch;
	
		fn get_highest_justified_epoch() -> Epoch;
	
		fn get_recommended_source_epoch() -> Epoch;
	
		fn get_recommended_target_hash() -> <Block as BlockT>::Hash;

		fn get_last_finalized_hash() -> <Block as BlockT>::Hash;
	}
}
