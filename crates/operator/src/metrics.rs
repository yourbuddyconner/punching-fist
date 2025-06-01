use lazy_static::lazy_static;
use prometheus::{register_int_counter, Encoder, IntCounter, Registry, TextEncoder};

lazy_static! {
    pub static ref REGISTRY: Registry = Registry::new();
    pub static ref PROCESSED_ALERTS_TOTAL: IntCounter = 
        register_int_counter!(
            "punchingfist_processed_alerts_total",
            "Total number of processed alerts."
        ).unwrap();
}

// Function to register metrics (though lazy_static handles this for PROCESSED_ALERTS_TOTAL)
// We can add more metrics here later and register them explicitly if needed.
pub fn register_metrics() {
    REGISTRY
        .register(Box::new(PROCESSED_ALERTS_TOTAL.clone()))
        .expect("Failed to register PROCESSED_ALERTS_TOTAL");
    // Add other metric registrations here if they are not using lazy_static register_... macros
}

// Function to gather metrics for exposition
pub fn gather_metrics() -> String {
    let mut buffer = vec![];
    let encoder = TextEncoder::new();
    let metric_families = REGISTRY.gather();
    encoder
        .encode(&metric_families, &mut buffer)
        .expect("Failed to encode metrics");
    String::from_utf8(buffer).expect("Failed to convert metrics to string")
} 