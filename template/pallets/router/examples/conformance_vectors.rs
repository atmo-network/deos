use codec::Encode;
use pallet_deos_router::{
  PreparedLeg, PreparedLegs, PreparedRoute, RouteFamily, RouteWeightClass, RouterOutcome,
};
use primitives::AssetKind;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{fs, path::PathBuf};

fn sha256(bytes: &[u8]) -> String {
  format!("{:x}", Sha256::digest(bytes))
}

fn hex(bytes: &[u8]) -> String {
  let mut output = String::with_capacity(bytes.len() * 2 + 2);
  output.push_str("0x");
  for byte in bytes {
    use std::fmt::Write;
    write!(output, "{byte:02x}").expect("writing to String cannot fail");
  }
  output
}

fn xyk_leg(
  asset_in: AssetKind,
  asset_out: AssetKind,
  amount_in: u128,
  amount_out: u128,
) -> PreparedLeg {
  let pool_id = if asset_in < asset_out {
    (asset_in, asset_out)
  } else {
    (asset_out, asset_in)
  };
  PreparedLeg::Xyk {
    pool_id,
    asset_in,
    asset_out,
    quoted_amount_in: amount_in,
    quoted_amount_out: amount_out,
  }
}

fn vector(
  name: &str,
  family: RouteFamily,
  weight_class: RouteWeightClass,
  legs: PreparedLegs,
  total_amount_in: u128,
  router_fee: u128,
  routed_amount_in: u128,
  recipient_amount_out: u128,
) -> Value {
  let prepared = PreparedRoute {
    family,
    legs: legs.clone(),
    total_amount_in,
    router_fee,
    routed_amount_in,
    recipient_amount_out,
    weight_class,
  };
  let outcome = RouterOutcome {
    family,
    legs,
    total_amount_in,
    router_fee,
    routed_amount_in,
    recipient_amount_out,
    weight_class,
  };
  json!({
    "name": name,
    "family": format!("{family:?}"),
    "weightClass": format!("{weight_class:?}"),
    "preparedScale": hex(&prepared.encode()),
    "outcomeScale": hex(&outcome.encode()),
    "crossDomainEqual": prepared.family == outcome.family
      && prepared.legs == outcome.legs
      && prepared.total_amount_in == outcome.total_amount_in
      && prepared.router_fee == outcome.router_fee
      && prepared.routed_amount_in == outcome.routed_amount_in
      && prepared.recipient_amount_out == outcome.recipient_amount_out
      && prepared.weight_class == outcome.weight_class,
    "totalAmountIn": total_amount_in.to_string(),
    "routerFee": router_fee.to_string(),
    "routedAmountIn": routed_amount_in.to_string(),
    "recipientAmountOut": recipient_amount_out.to_string(),
  })
}

fn main() {
  let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
  let repo = manifest.join("../../..");
  let specification = fs::read(manifest.join("docs/specification.en.md"))
    .expect("Router specification must be readable");
  let metadata = fs::read(repo.join("web-client/.papi/metadata/deos.scale"))
    .expect("runtime metadata must be generated");
  let router_weights = fs::read(repo.join("template/runtime/src/weights/pallet_deos_router.rs"))
    .expect("Router weights must be generated");
  let actor_weights = fs::read(repo.join("template/runtime/src/weights/pallet_deos_actors.rs"))
    .expect("Actors weights must be generated");

  let foreign_a = AssetKind::Local(2);
  let foreign_b = AssetKind::Local(3);
  let native = AssetKind::Native;
  let fee = 5u128;
  let routed = 995u128;

  let vectors = vec![
    vector(
      "exact-input-direct-xyk",
      RouteFamily::DirectXyk,
      RouteWeightClass::ExactInputDirectXyk,
      vec![xyk_leg(foreign_a, native, routed, 900)]
        .try_into()
        .unwrap(),
      1_000,
      fee,
      routed,
      900,
    ),
    vector(
      "exact-input-direct-mint",
      RouteFamily::DirectMint,
      RouteWeightClass::ExactInputDirectMint,
      vec![PreparedLeg::TmcMint {
        token_asset: native,
        collateral_asset: foreign_a,
        quoted_collateral_in: routed,
        quoted_recipient_out: 800,
      }]
      .try_into()
      .unwrap(),
      1_000,
      fee,
      routed,
      800,
    ),
    vector(
      "exact-input-native-anchored-xyk",
      RouteFamily::NativeAnchoredXyk,
      RouteWeightClass::ExactInputNativeAnchoredXyk,
      vec![
        xyk_leg(foreign_a, native, routed, 920),
        xyk_leg(native, foreign_b, 920, 850),
      ]
      .try_into()
      .unwrap(),
      1_000,
      fee,
      routed,
      850,
    ),
    vector(
      "exact-output-direct-xyk",
      RouteFamily::DirectXyk,
      RouteWeightClass::ExactOutputDirectXyk,
      vec![xyk_leg(foreign_a, native, 900, 800)]
        .try_into()
        .unwrap(),
      905,
      fee,
      900,
      800,
    ),
    vector(
      "exact-output-native-anchored-xyk",
      RouteFamily::NativeAnchoredXyk,
      RouteWeightClass::ExactOutputNativeAnchoredXyk,
      vec![
        xyk_leg(foreign_a, native, 950, 900),
        xyk_leg(native, foreign_b, 900, 800),
      ]
      .try_into()
      .unwrap(),
      955,
      fee,
      950,
      800,
    ),
  ];

  assert_eq!(vectors.len(), 5);
  let output = serde_json::to_string_pretty(&json!({
    "format": "deos.router.conformance-vectors",
    "formatVersion": 1,
    "specificationSha256": sha256(&specification),
    "metadataSha256": sha256(&metadata),
    "routerWeightsSha256": sha256(&router_weights),
    "actorWeightsSha256": sha256(&actor_weights),
    "vectors": vectors,
  }))
  .expect("vector JSON must serialize");
  if std::env::args().any(|argument| argument == "--check") {
    let accepted =
      fs::read_to_string(manifest.join("tests/fixtures/router-conformance-vectors.v1.json"))
        .expect("accepted vector fixture must be readable");
    assert_eq!(accepted.trim_end(), output);
  } else {
    println!("{output}");
  }
}
