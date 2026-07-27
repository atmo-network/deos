use polkadot_sdk::frame_support::weights::Weight;

pub trait WeightInfo {
  fn register_feed_existing_producer() -> Weight;
  fn register_feed_new_producer() -> Weight;
  fn pause_feed() -> Weight;
  fn resume_feed() -> Weight;
  fn deactivate_feed() -> Weight;
  fn publish_last_value() -> Weight;
  fn publish_ema_changed() -> Weight;
  fn publish_ema_refresh() -> Weight;
}

impl WeightInfo for () {
  fn register_feed_existing_producer() -> Weight {
    Weight::from_parts(150_000_000, 35_000)
  }

  fn register_feed_new_producer() -> Weight {
    Weight::from_parts(150_000_000, 35_000)
  }

  fn pause_feed() -> Weight {
    Weight::from_parts(20_000_000, 4_000)
  }

  fn resume_feed() -> Weight {
    Weight::from_parts(20_000_000, 4_000)
  }

  fn deactivate_feed() -> Weight {
    Weight::from_parts(20_000_000, 4_000)
  }

  fn publish_last_value() -> Weight {
    Weight::from_parts(35_000_000, 6_000)
  }

  fn publish_ema_changed() -> Weight {
    Weight::from_parts(45_000_000, 6_000)
  }

  fn publish_ema_refresh() -> Weight {
    Weight::from_parts(45_000_000, 6_000)
  }
}
