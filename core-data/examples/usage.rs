use std::fs;
use std::time::Instant;
use core_data::models::message::*;
use serde_json::json;

fn main() {
    let xml_bytes = fs::read("examples/pacs008_001_07_cct_outgoing.xml")
        .expect("Failed to read XML file");

    let payload = Payload::new_inline(
        Some(xml_bytes.clone()),
        PayloadFormat::Xml,
        PayloadSchema::ISO20022,
        Encoding::Utf8,
    );

    let mut message = Message::new(
        payload,
        "banking".to_string(),
        "pacs.008.001.07".to_string(),
        "UsageExample".to_string(),
        1,
        "ISOOutgoing".to_string(),
        Some("Payment".to_string()),
    );

    // Time the parse operation
    let parse_start = Instant::now();
    message.parse(Some("Parsed pacs.008 message".to_string()), "UsageExample".to_string(), 1, "ISOOutgoing".to_string())
        .expect("Failed to parse XML message");
    let parse_duration = parse_start.elapsed();

    let enrichment_config = vec![
        EnrichmentRules {
            field: "data.metadata.processing_date".to_string(),
            logic: json!({"var": ["processing_date"]}),
            description: Some("Add processing date".to_string()),
        },
        EnrichmentRules {
            field: "data.metadata.transaction_type".to_string(),
            logic: json!({"var": ["transaction_type"]}),
            description: Some("Add transaction type".to_string()),
        },
    ];

    let enrichment_data = json!({
        "processing_date": "2024-01-18T10:30:00Z",
        "transaction_type": "INSTANT_CREDIT_TRANSFER"
    });

    // Time the enrich operation
    let enrich_start = Instant::now();
    message.enrich(enrichment_config, enrichment_data, Some("Enriched pacs.008 message".to_string()), "UsageExample".to_string(), 1, "ISOOutgoing".to_string())
        .expect("Failed to enrich message");
    let enrich_duration = enrich_start.elapsed();

    // Print timing results
    println!("\nPerformance Metrics:");
    println!("Parse time: {:?}", parse_duration);
    println!("Enrich time: {:?}", enrich_duration);
    println!("Total processing time: {:?}", parse_duration + enrich_duration);

    println!("\nParsed and Enriched Message:");
    println!("{}", serde_json::to_string_pretty(&message.data()).unwrap());

    println!("\nAudit Trail:");
    for audit in message.audit() {
        println!("Description: {}", audit.description());
        for change in audit.changes() {
            println!("  Field: {}", change.field());
            println!("  Old Value: {:?}", change.old_value());
            println!("  New Value: {:?}", change.new_value());
            println!("  Reason: {}", change.reason());
        }
    }
}