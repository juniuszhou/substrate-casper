#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::too_many_arguments)]

// Here we declare the runtime API. It is implemented it the `impl` block in
// runtime amalgamator file (the `runtime/src/lib.rs`)
sp_api::decl_runtime_apis! {

	pub trait CasperRuntimeApi<Epoch, Hash> where
		Epoch: codec::Codec,
		Hash: codec::Codec, {
		fn get_highest_finalized_epoch() -> Epoch;
	
		fn get_highest_justified_epoch() -> Epoch;
	
		fn get_recommended_source_epoch() -> Epoch;
	
		fn get_recommended_target_hash() -> Hash;
	}

}
