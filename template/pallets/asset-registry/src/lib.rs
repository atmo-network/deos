//! Asset Registry Pallet
//!
//! Manages foreign asset registration and XCM location mappings.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub use pallet::*;

pub mod weights;
pub use weights::WeightInfo;

pub trait TokenDomainHook {
  fn on_token_registered(
    token_asset: primitives::AssetKind,
  ) -> polkadot_sdk::sp_runtime::DispatchResult;
}

impl TokenDomainHook for () {
  fn on_token_registered(
    _token_asset: primitives::AssetKind,
  ) -> polkadot_sdk::sp_runtime::DispatchResult {
    Ok(())
  }
}

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

#[frame::pallet]
pub mod pallet {
  use crate::{TokenDomainHook as _, weights::WeightInfo as _};
  use frame::prelude::*;
  use polkadot_sdk::{
    frame_support::{
      traits::{EnsureOrigin, Get, fungibles::Inspect},
      transactional,
    },
    pallet_assets,
    sp_runtime::{
      DispatchResult,
      traits::{Convert, MaybeEquivalence, StaticLookup},
    },
    staging_xcm::latest::Location,
  };
  use primitives::assets::{CurrencyMetadata, MASK_TYPE, TYPE_FOREIGN};

  #[cfg(not(feature = "std"))]
  use alloc::vec::Vec;
  #[cfg(feature = "std")]
  use std::vec::Vec;

  #[pallet::config]
  pub trait Config: frame_system::Config + pallet_assets::Config {
    /// Origin that can register foreign assets (e.g. Governance or Root)
    type RegistryOrigin: EnsureOrigin<Self::RuntimeOrigin>;

    /// Strategy to generate a deterministic AssetId from a Location for NEW registrations.
    /// This is used only once upon registration. Afterward, the ID is persisted in storage.
    type AssetIdGenerator: Convert<Location, Self::AssetId>;

    /// The account that will own the created assets (usually a governance or system account)
    type AssetOwner: Get<Self::AccountId>;

    /// Hook for token-domain bootstrap logic in runtime glue
    type TokenDomainHook: crate::TokenDomainHook;

    type WeightInfo: crate::weights::WeightInfo;
  }

  #[pallet::pallet]
  #[pallet::storage_version(STORAGE_VERSION)]
  pub struct Pallet<T>(_);

  const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

  #[pallet::hooks]
  impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T>
  where
    <T as polkadot_sdk::pallet_assets::Config>::AssetId: Into<u32> + Copy,
  {
    #[cfg(feature = "try-runtime")]
    fn try_state(_n: BlockNumberFor<T>) -> Result<(), polkadot_sdk::sp_runtime::TryRuntimeError> {
      Self::do_try_state()
    }
  }

  /// Mapping from XCM Location to Local AssetId.
  /// This serves as the source of truth for foreign asset identification.
  #[pallet::storage]
  #[pallet::getter(fn location_to_asset)]
  pub type ForeignAssetMapping<T: Config> =
    StorageMap<_, Blake2_128Concat, Location, T::AssetId, OptionQuery>;

  #[pallet::event]
  #[pallet::generate_deposit(pub(super) fn deposit_event)]
  pub enum Event<T: Config> {
    /// A foreign asset has been registered.
    ForeignAssetRegistered {
      asset_id: T::AssetId,
      location: Location,
      symbol: Vec<u8>,
    },
    /// A mapping key was migrated (e.g., XCM version update).
    MigrationApplied {
      asset_id: T::AssetId,
      old_location: Location,
      new_location: Location,
    },
  }

  #[pallet::error]
  pub enum Error<T> {
    /// The asset is already registered.
    AssetAlreadyRegistered,
    /// The generated AssetId is already in use by another asset.
    AssetIdCollision,
    /// The AssetId does not have the correct FOREIGN mask (0xF...).
    InvalidAssetIdMask,
    /// The asset does not exist in pallet-assets.
    AssetNotFound,
    /// Asset name or symbol exceeds the allowed length limit.
    MetadataTooLong,
  }

  #[pallet::call]
  impl<T: Config> Pallet<T>
  where
    <T as polkadot_sdk::pallet_assets::Config>::AssetId: Into<u32> + Copy,
    <T as polkadot_sdk::pallet_assets::Config>::AssetIdParameter:
      From<<T as polkadot_sdk::pallet_assets::Config>::AssetId> + Copy,
  {
    /// Register a foreign asset.
    ///
    /// This derives the deterministic `AssetId` from the XCM `Location` using `AssetIdGenerator`,
    /// persists the mapping `Location -> AssetId`, and then creates the asset in `pallet-assets`.
    ///
    /// - `origin`: Must match `RegistryOrigin`.
    /// - `location`: The XCM location of the asset.
    /// - `metadata`: Name, Symbol, and Decimals.
    /// - `min_balance`: The minimum balance (ED) for the asset.
    /// - `is_sufficient`: Whether the asset can be used to pay fees (requires `pallet-assets` support).
    #[pallet::call_index(0)]
    #[pallet::weight(<T as crate::pallet::Config>::WeightInfo::register_foreign_asset())]
    #[transactional]
    pub fn register_foreign_asset(
      origin: OriginFor<T>,
      location: Location,
      metadata: CurrencyMetadata,
      min_balance: T::Balance,
      is_sufficient: bool,
    ) -> DispatchResult {
      T::RegistryOrigin::ensure_origin(origin)?;
      // 0. Validate metadata
      Self::validate_metadata(&metadata)?;
      // 1. Check if already registered
      ensure!(
        !ForeignAssetMapping::<T>::contains_key(&location),
        Error::<T>::AssetAlreadyRegistered
      );
      // 2. Generate Deterministic AssetId
      let asset_id = T::AssetIdGenerator::convert(location.clone());
      // 3. Check for ID collision
      if pallet_assets::Pallet::<T>::asset_exists(asset_id) {
        return Err(Error::<T>::AssetIdCollision.into());
      }
      // 4. Persist Mapping
      ForeignAssetMapping::<T>::insert(&location, &asset_id);
      // 5. Prepare Asset Owner
      // `pallet-assets` requires the owner to be passed as a Source (lookup target)
      let owner = T::AssetOwner::get();
      let owner_source = T::Lookup::unlookup(owner);
      // 6. Create Asset via Root (Force Create)
      // We use Root origin because `force_create` requires it.
      // This implies trust in `RegistryOrigin` to trigger this.
      pallet_assets::Pallet::<T>::force_create(
        frame_system::RawOrigin::Root.into(),
        <T as polkadot_sdk::pallet_assets::Config>::AssetIdParameter::from(asset_id),
        owner_source,
        is_sufficient,
        min_balance,
      )?;
      // 7. Set Metadata via Root
      pallet_assets::Pallet::<T>::force_set_metadata(
        frame_system::RawOrigin::Root.into(),
        <T as polkadot_sdk::pallet_assets::Config>::AssetIdParameter::from(asset_id),
        metadata.name,
        metadata.symbol.clone(),
        metadata.decimals,
        false, // is_frozen
      )?;
      // 8. Notify token-domain hook
      T::TokenDomainHook::on_token_registered(primitives::AssetKind::Foreign(asset_id.into()))?;
      // 9. Emit Event
      Self::deposit_event(Event::ForeignAssetRegistered {
        asset_id,
        location,
        symbol: metadata.symbol,
      });

      Ok(())
    }

    /// Register a foreign asset with a specific ID.
    ///
    /// Useful for migrations or resolving collisions manually.
    /// The ID must strictly follow the Foreign Asset bitmask (0xF...).
    #[pallet::call_index(1)]
    #[pallet::weight(<T as crate::pallet::Config>::WeightInfo::register_foreign_asset_with_id())]
    #[transactional]
    pub fn register_foreign_asset_with_id(
      origin: OriginFor<T>,
      location: Location,
      asset_id: T::AssetId,
      metadata: CurrencyMetadata,
      min_balance: T::Balance,
      is_sufficient: bool,
    ) -> DispatchResult {
      T::RegistryOrigin::ensure_origin(origin)?;
      // 0. Validate metadata
      Self::validate_metadata(&metadata)?;
      // 1. Validate Mask
      let id_u32: u32 = asset_id.into();
      ensure!(
        (id_u32 & MASK_TYPE) == TYPE_FOREIGN,
        Error::<T>::InvalidAssetIdMask
      );
      // 2. Check if already registered
      ensure!(
        !ForeignAssetMapping::<T>::contains_key(&location),
        Error::<T>::AssetAlreadyRegistered
      );
      // 3. Check for ID collision
      if pallet_assets::Pallet::<T>::asset_exists(asset_id) {
        return Err(Error::<T>::AssetIdCollision.into());
      }
      // 4. Persist Mapping
      ForeignAssetMapping::<T>::insert(&location, &asset_id);
      // 5. Prepare Asset Owner
      let owner = T::AssetOwner::get();
      let owner_source = T::Lookup::unlookup(owner);
      // 6. Create Asset via Root (Force Create)
      pallet_assets::Pallet::<T>::force_create(
        frame_system::RawOrigin::Root.into(),
        <T as polkadot_sdk::pallet_assets::Config>::AssetIdParameter::from(asset_id),
        owner_source,
        is_sufficient,
        min_balance,
      )?;
      // 7. Set Metadata via Root
      pallet_assets::Pallet::<T>::force_set_metadata(
        frame_system::RawOrigin::Root.into(),
        <T as polkadot_sdk::pallet_assets::Config>::AssetIdParameter::from(asset_id),
        metadata.name,
        metadata.symbol.clone(),
        metadata.decimals,
        false, // is_frozen
      )?;
      // 8. Notify token-domain hook
      T::TokenDomainHook::on_token_registered(primitives::AssetKind::Foreign(asset_id.into()))?;
      // 9. Emit Event
      Self::deposit_event(Event::ForeignAssetRegistered {
        asset_id,
        location,
        symbol: metadata.symbol,
      });
      Ok(())
    }

    /// Link an existing asset to a foreign location.
    ///
    /// Useful if the asset was created manually via `force_create` and now needs XCM binding.
    /// The AssetId must exist and have the correct FOREIGN mask.
    #[pallet::call_index(2)]
    #[pallet::weight(<T as crate::pallet::Config>::WeightInfo::link_existing_asset(
      ForeignAssetMapping::<T>::iter_keys().count() as u32
    ))]
    pub fn link_existing_asset(
      origin: OriginFor<T>,
      location: Location,
      asset_id: T::AssetId,
    ) -> DispatchResult {
      T::RegistryOrigin::ensure_origin(origin)?;
      // 1. Validate Mask
      let id_u32: u32 = asset_id.into();
      ensure!(
        (id_u32 & MASK_TYPE) == TYPE_FOREIGN,
        Error::<T>::InvalidAssetIdMask
      );
      // 2. Check if already registered
      ensure!(
        !ForeignAssetMapping::<T>::contains_key(&location),
        Error::<T>::AssetAlreadyRegistered
      );
      // 3. Check existence in pallet-assets
      ensure!(
        pallet_assets::Pallet::<T>::asset_exists(asset_id),
        Error::<T>::AssetNotFound
      );
      // 4. Bijectivity guard: ensure asset_id is not already mapped to a different location
      for (_loc, mapped_id) in ForeignAssetMapping::<T>::iter() {
        if mapped_id == asset_id {
          return Err(Error::<T>::AssetAlreadyRegistered.into());
        }
      }
      // 5. Persist Mapping
      ForeignAssetMapping::<T>::insert(&location, &asset_id);
      // 6. Notify token-domain hook
      T::TokenDomainHook::on_token_registered(primitives::AssetKind::Foreign(asset_id.into()))?;
      // 7. Emit Event (Reuse registration event as the outcome is the same: mapping created)
      let symbol = pallet_assets::Metadata::<T>::get(asset_id)
        .symbol
        .into_inner();
      Self::deposit_event(Event::ForeignAssetRegistered {
        asset_id,
        location,
        symbol,
      });
      Ok(())
    }

    /// Migrate a mapping key (e.g., XCM version upgrade) without changing AssetId.
    #[pallet::call_index(3)]
    #[pallet::weight(<T as crate::pallet::Config>::WeightInfo::migrate_location_key())]
    pub fn migrate_location_key(
      origin: OriginFor<T>,
      old_location: Location,
      new_location: Location,
    ) -> DispatchResult {
      T::RegistryOrigin::ensure_origin(origin)?;
      let asset_id =
        ForeignAssetMapping::<T>::get(&old_location).ok_or(Error::<T>::AssetNotFound)?;
      ensure!(
        !ForeignAssetMapping::<T>::contains_key(&new_location),
        Error::<T>::AssetAlreadyRegistered
      );
      ForeignAssetMapping::<T>::remove(&old_location);
      ForeignAssetMapping::<T>::insert(&new_location, &asset_id);
      Self::deposit_event(Event::MigrationApplied {
        asset_id,
        old_location,
        new_location,
      });
      Ok(())
    }
  }

  impl<T: Config> Pallet<T> {
    fn validate_metadata(metadata: &CurrencyMetadata) -> DispatchResult {
      let limit = <T as pallet_assets::Config>::StringLimit::get() as usize;
      ensure!(metadata.name.len() <= limit, Error::<T>::MetadataTooLong);
      ensure!(metadata.symbol.len() <= limit, Error::<T>::MetadataTooLong);
      Ok(())
    }
  }

  /// Implementation of Convert trait to be used by xcm_config for LocationToAssetId lookup.
  impl<T: Config> Convert<Location, Option<T::AssetId>> for Pallet<T> {
    fn convert(location: Location) -> Option<T::AssetId> {
      ForeignAssetMapping::<T>::get(location)
    }
  }

  impl<T: Config> MaybeEquivalence<Location, T::AssetId> for Pallet<T> {
    fn convert(location: &Location) -> Option<T::AssetId> {
      ForeignAssetMapping::<T>::get(location)
    }

    fn convert_back(_: &T::AssetId) -> Option<Location> {
      None
    }
  }

  #[cfg(feature = "try-runtime")]
  impl<T: Config> Pallet<T>
  where
    <T as polkadot_sdk::pallet_assets::Config>::AssetId: Into<u32> + Copy,
  {
    pub(crate) fn do_try_state() -> Result<(), polkadot_sdk::sp_runtime::TryRuntimeError> {
      use alloc::collections::BTreeSet;
      use polkadot_sdk::sp_runtime::TryRuntimeError;
      let mut seen_ids = BTreeSet::new();
      let mut mapping_iter = ForeignAssetMapping::<T>::iter();
      while let Some((location, asset_id)) = mapping_iter.next() {
        // Invariant 1: No two locations map to the same AssetId (bijectivity)
        let id_u32: u32 = asset_id.into();
        if !seen_ids.insert(id_u32) {
          return Err(TryRuntimeError::Other(
            "ForeignAssetMapping: duplicate AssetId for different Locations",
          ));
        }
        // Invariant 2: Every mapped AssetId has the FOREIGN bitmask
        if (id_u32 & primitives::assets::MASK_TYPE) != primitives::assets::TYPE_FOREIGN {
          return Err(TryRuntimeError::Other(
            "ForeignAssetMapping: AssetId does not have FOREIGN bitmask",
          ));
        }
        // Invariant 3: Every mapped AssetId exists in pallet-assets
        if !pallet_assets::Pallet::<T>::asset_exists(asset_id) {
          return Err(TryRuntimeError::Other(
            "ForeignAssetMapping: AssetId does not exist in pallet-assets",
          ));
        }
        let _ = location;
      }
      Ok(())
    }
  }
}
