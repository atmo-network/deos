/*
Domain: Certified AddressEvent ingress
Owns: Generated expected certified-producer inventory and typed-boundary evidence.
Excludes: Live chain evidence, observation state, and estimate projection.
Zone: Generated ingress domain evidence; regenerate through the owning script.
*/
export const DEOS_INGRESS_RUNTIME_EVIDENCE = {
  runtime: {
    specName: 'deos-runtime',
    implName: 'deos-runtime',
    authoringVersion: 1,
    specVersion: 1,
    implVersion: 1,
    systemVersion: 3,
    transactionVersion: 1,
  },
  inventorySha256:
    'dbbedcbf1c0f6f035348ac80b4624c3a10dc43a5df90d0c2a7d3dac1468340fa',
  certifiedProducers: [
    {
      id: 'AddressEventIngressExtension::signed_transfer',
      creditedSurface: 'Recipient sovereign',
      sourceProvenance: 'Signer / Signed',
      preflightOwner: 'TransactionExtension::prepare',
      notifyOwner: 'TransactionExtension::post_dispatch_details',
      rollbackOwner: 'Balances/Assets ledger revert',
      weightOwner: 'transaction_extension_ingress_base/_notify',
    },
    {
      id: 'AddressEventIngressExtension::transfer_all',
      creditedSurface: 'Recipient sovereign',
      sourceProvenance: 'Signer / Signed, actual recipient delta',
      preflightOwner: 'TransactionExtension::prepare',
      notifyOwner: 'TransactionExtension::post_dispatch_details',
      rollbackOwner: 'Balances ledger revert',
      weightOwner: 'transaction_extension_ingress_base/_notify',
    },
    {
      id: 'TmctolAssetOps::transfer',
      creditedSurface: 'Task `to` sovereign',
      sourceProvenance: 'Sender / InternalProtocol',
      preflightOwner: 'TmctolAssetOps::transfer preflight',
      notifyOwner: 'RuntimeAddressEventIngress::on_internal_inbound',
      rollbackOwner: 'Asset ops storage transaction',
      weightOwner: 'task_transfer/task_split_transfer generated weights',
    },
    {
      id: 'TmctolAssetOps::mint',
      creditedSurface: 'Task `to` sovereign',
      sourceProvenance: 'Source-less / none',
      preflightOwner: 'Source-less preflight inside notify',
      notifyOwner: 'RuntimeAddressEventIngress::on_inbound_without_source',
      rollbackOwner: 'Asset ops storage transaction',
      weightOwner: 'task_mint generated weight',
    },
    {
      id: 'TmctolMintDistributionIngress',
      creditedSurface: 'Collateral/minted recipients',
      sourceProvenance: 'Mint source / InternalProtocol',
      preflightOwner: 'before_collateral_transfer/before_sink_mint',
      notifyOwner: 'after_distribution',
      rollbackOwner: 'TMC distribution transaction',
      weightOwner: 'TMC distribution generated weights',
    },
    {
      id: 'DeosRouter::route_fee',
      creditedSurface: 'Burn Actor sovereign',
      sourceProvenance: 'Fee payer / InternalProtocol',
      preflightOwner: 'FeeManagerImpl::route_fee preflight',
      notifyOwner: 'FeeManagerImpl::route_fee notify',
      rollbackOwner: 'Router fee transaction',
      weightOwner: 'Router fee routing generated weights',
    },
    {
      id: 'XCM asset deposit',
      creditedSurface: 'Recipient sovereign',
      sourceProvenance: 'XCM origin / Xcm',
      preflightOwner: 'ActorAwareAssetTransactor::preflight_ingress',
      notifyOwner: 'ActorAwareAssetTransactor::notify_ingress',
      rollbackOwner: 'ActorAwareAssetTransactor deposit revert',
      weightOwner: 'One-asset deposit generated weight',
    },
    {
      id: 'XCM deposit without origin',
      creditedSurface: 'Recipient sovereign',
      sourceProvenance: 'Source-less / none',
      preflightOwner: 'Source-less preflight inside notify',
      notifyOwner: 'ActorAwareAssetTransactor::on_inbound_without_source',
      rollbackOwner: 'ActorAwareAssetTransactor deposit revert',
      weightOwner: 'One-asset deposit generated weight',
    },
  ],
  boundary: {
    typedTrait: 'pallet_deos_actors::AddressEventIngress',
    adapter: 'RuntimeAddressEventIngress',
    extension: 'AddressEventIngressExtension',
    helperFiles: [
      'actor_config.rs',
      'address_event_ingress.rs',
      'deos_router_config.rs',
      'tmc_config.rs',
      'xcm_config.rs',
    ],
  },
} as const;
