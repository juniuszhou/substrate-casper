#![cfg_attr(not(feature = "std"), no_std)]

use sp_runtime::traits::Block as BlockT;

// Here we declare the runtime API. It is implemented it the `impl` block in
// runtime amalgamator file (the `runtime/src/lib.rs`)
// Reference as AuraApi
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
