use std::fs;
use core_data::models::message::*;
use serde_json::json;

#[test]
fn test_message_lifecycle() {
    // Stage 1: Create message with XML payload
    let xml_bytes = fs::read("examples/pacs008_001_07_cct_outgoing.xml")
        .expect("Failed to read test XML file");
        
    let payload = Payload::new_inline(
        Some(xml_bytes),
        PayloadFormat::Xml, 
        PayloadSchema::ISO20022,
        Encoding::Utf8
    );

    let mut message = Message::new(
        payload,
        "banking".to_string(),
        "pacs.008.001.07".to_string(), 
        "test_message_lifecycle".to_string(),
        "ISOOutgoing".to_string(),
        Some("payment".to_string())
    );

    // Verify initial state
    assert!(message.data().is_null());
    assert_eq!(message.audit().len(), 1);

    // Stage 2: Parse ISO20022 XML
    message.parse(Some("Parsed payment message".to_string()),
        "test_message_lifecycle".to_string(),
        "ISOOutgoing".to_string(),
    ).expect("Failed to parse message");

    // Verify parsed state
    assert!(!message.data().is_null());
    assert_eq!(message.audit().len(), 2);
    assert!(message.data()["document"]["FIToFICstmrCdtTrf"]["GrpHdr"]["MsgId"]
        .as_str()
        .unwrap()
        .contains("VOLCUSTMSGID"));

    // Stage 3: Enrich message
    let enrichment_config = vec![
        EnrichmentConfig {
            field: "data.metadata.processing_date".to_string(),
            rule: json!({"var": ["processing_date"]}),
            description: Some("Add processing timestamp".to_string()),
        },
        EnrichmentConfig {
            field: "data.metadata.message_type".to_string(),
            rule: json!({"var": ["message_type"]}),
            description: Some("Add message classification".to_string()),
        }
    ];

    let enrichment_data = json!({
        "processing_date": "2024-01-18T12:00:00Z",
        "message_type": "CREDIT_TRANSFER"
    });

    message.enrich(
        enrichment_config,
        enrichment_data,
        Some("Applied metadata enrichment".to_string()),
        "test_message_lifecycle".to_string(),
        "ISOOutgoing".to_string(),
    ).expect("Failed to enrich message");

    // Verify final enriched state
    assert_eq!(message.audit().len(), 3);
    assert_eq!(
        message.data()["metadata"]["processing_date"],
        "2024-01-18T12:00:00Z"
    );
    assert_eq!(
        message.data()["metadata"]["message_type"],
        "CREDIT_TRANSFER"
    );
    
    // Verify complete audit trail
    let audit_trail = message.audit();
    assert_eq!(audit_trail[0].description(), "Payment created");
    assert_eq!(audit_trail[1].description(), "Parsed payment message");
    assert_eq!(audit_trail[2].description(), "Applied metadata enrichment");
}