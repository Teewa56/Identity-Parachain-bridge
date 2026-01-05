#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

pub mod weights;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

#[frame::pallet]
pub mod pallet {
	use frame::prelude::*;
	use frame::support::traits::EnsureOrigin;
	use sp_core::H256;
	use sp_runtime::traits::Hash;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type WeightInfo: crate::weights::WeightInfo;
		/// The origin that can manage the whitelist of issuers and schemas.
		type GovernanceOrigin: EnsureOrigin<Self::RuntimeOrigin>;
		/// The currency trait for fee handling.
		type Currency: frame_support::traits::fungible::Mutate<Self::AccountId>;
		/// The amount of currency required to issue a new identity(constant fee)
		#[pallet::constant]
		type IssuanceFee: Get<BalanceOf<Self>>;
		/// The account that receives the issuance fees (constant e.g., Treasury).
		#[pallet::constant]
		type FeeReceiver: Get<Self::AccountId>;
	}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	type BalanceOf<T> = <<T as Config>::Currency as frame_support::traits::fungible::Inspect<
		<T as frame_system::Config>::AccountId,
	>>::Balance;

	/// A single identity attestation leaf.
	#[derive(Encode, Decode, MaxEncodedLen, TypeInfo, Clone, PartialEq, Eq, RuntimeDebug)]
	pub struct AttestationLeaf {
		pub did: H256,
		pub schema_hash: H256,
		pub data_hash: H256,
		pub expiration: Option<u64>,
	}

	/// Map of leaf index to its hash in the MMR(Merkle mountain range)
	#[pallet::storage]
	pub type MMRNodes<T: Config> = StorageMap<_, Blake2_128Concat, u64, H256>;

	/// The total number of leaves in the MMR.
	#[pallet::storage]
	pub type MMRSize<T: Config> = StorageValue<_, u64, ValueQuery>;

	/// Current status and MMR leaf index of a DID.
	#[pallet::storage]
	pub type IdentityStatus<T: Config> = StorageMap<_, Blake2_128Concat, H256, (bool, u64), ValueQuery>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A new identity attestation has been issued.
		IdentityIssued { did: H256, index: u64, root: H256 },
		/// An identity has been revoked.
		IdentityRevoked { did: H256 },
		/// A new issuer has been whitelisted(Like an oragnisation )
		IssuerAdded { issuer: T::AccountId },
		/// A new schema has been whitelisted.
		SchemaAdded { hash: H256 },
		/// A fee has been paid for identity issuance.
		FeePaid { who: T::AccountId, amount: BalanceOf<T> },
	}

	#[pallet::error]
	pub enum Error<T> {
		/// The identity has already been issued.
		AlreadyExists,
		/// The identity was not found.
		NotFound,
		/// The caller is not an authorized issuer.
		NotAuthorizedIssuer,
		/// The schema is not recognized or whitelisted.
		InvalidSchema,
		/// Not enough funds to pay the issuance fee.
		InsufficientBalance,
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Issues a new identity attestation. Only whitelisted issuers.
		#[pallet::call_index(0)]
		#[pallet::weight(Weight::from_parts(10_000, 0) + T::DbWeight::get().reads_writes(5, 4))]
		pub fn issue_identity(
			origin: OriginFor<T>,
			did: H256,
			schema_hash: H256,
			data_hash: H256,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			// Security Check 1: Issuer Authorization
			ensure!(<WhitelistedIssuers<T>>::get(&who), Error::<T>::NotAuthorizedIssuer);

			// Security Check 2: Schema Validation
			ensure!(<WhitelistedSchemas<T>>::get(&schema_hash), Error::<T>::InvalidSchema);

			ensure!(!<IdentityStatus<T>>::get(did).0, Error::<T>::AlreadyExists);

			// Economic Check: Pay the Fee
			let fee = T::IssuanceFee::get();
			T::Currency::transfer(&who, &T::FeeReceiver::get(), fee, frame_support::traits::tokens::Preservation::Preserve)?;
			
			let leaf = AttestationLeaf {
				did,
				schema_hash,
				data_hash,
				expiration: None,
			};

			let leaf_hash = T::Hashing::hash_of(&leaf);
			let index = <MMRSize<T>>::get();

			// Cryptographic MMR Update (Industrial logic placeholder)
			// Using sp-mmr compatible indexing (0-based leaf index)
			<MMRNodes<T>>::insert(index, leaf_hash);
			<MMRSize<T>>::put(index + 1);
			<IdentityStatus<T>>::insert(did, (true, index));

			// Root Calculation: In a production ZK-bridge, this is the MMR Peak Hash.
			// Placeholder for the Noir circuit to verify against.
			let root = leaf_hash; 

			Self::deposit_event(Event::FeePaid { who: who.clone(), amount: fee });
			Self::deposit_event(Event::IdentityIssued { did, index, root });

			Ok(())
		}

		/// Whitelist a new issuer. Only Governance/Root.
		#[pallet::call_index(2)]
		#[pallet::weight(Weight::from_parts(10_000, 0) + T::DbWeight::get().writes(1))]
		pub fn add_issuer(origin: OriginFor<T>, issuer: T::AccountId) -> DispatchResult {
			T::GovernanceOrigin::ensure_origin(origin)?;
			<WhitelistedIssuers<T>>::insert(&issuer, true);
			Self::deposit_event(Event::IssuerAdded { issuer });
			Ok(())
		}

		/// Register a new validated VC schema. Only Governance/Root.
		#[pallet::call_index(3)]
		#[pallet::weight(Weight::from_parts(10_000, 0) + T::DbWeight::get().writes(1))]
		pub fn add_schema(origin: OriginFor<T>, schema_hash: H256) -> DispatchResult {
			T::GovernanceOrigin::ensure_origin(origin)?;
			<WhitelistedSchemas<T>>::insert(&schema_hash, true);
			Self::deposit_event(Event::SchemaAdded { hash: schema_hash });
			Ok(())
		}

		/// Revokes an existing identity.
		#[pallet::call_index(4)]
		#[pallet::weight(Weight::from_parts(10_000, 0) + T::DbWeight::get().writes(1))]
		pub fn revoke_identity(origin: OriginFor<T>, did: H256) -> DispatchResult {
			let who = ensure_signed(origin)?;
			// For now, only the issuer or root can revoke. Simple issuer check:
			ensure!(<WhitelistedIssuers<T>>::get(&who), Error::<T>::NotAuthorizedIssuer);
			ensure!(<IdentityStatus<T>>::get(did), Error::<T>::NotFound);

			<IdentityStatus<T>>::insert(did, false);
			Self::deposit_event(Event::IdentityRevoked { did });
			Ok(())
		}
	}
}
