use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct Payload {
    /// Storage type: inline or file
    storage: StorageType,
    
    /// Actual content when stored inline
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<Box<[u8]>>,
    
    /// URL for external content
    #[serde(skip_serializing_if = "Option::is_none")]
    url: Option<Box<str>>,
    
    /// Content format
    format: PayloadFormat,

    schema: PayloadSchema,
    
    /// Character encoding
    encoding: Encoding,
    
    /// Size in bytes
    size: i64,
}

impl Payload {
    pub fn content(&self) -> Option<&[u8]> {
        self.content.as_deref()
    }

    pub fn url(&self) -> Option<&str> {
        self.url.as_deref()
    }

    pub fn new_inline(content: Option<Vec<u8>>, format: PayloadFormat, schema: PayloadSchema, encoding: Encoding) -> Self {
        let content = content.map(|v| v.into_boxed_slice());
        Self {
            storage: StorageType::Inline,
            content,
            url: None,
            format,
            schema,
            encoding,
            size: 0,
        }
    }

    pub fn new_file<S: Into<Box<str>>>(url: Option<S>, format: PayloadFormat, schema: PayloadSchema, encoding: Encoding, size: i64) -> Self {
        Self {
            storage: StorageType::File,
            content: None,
            url: url.map(|s| s.into()),
            format,
            schema,
            encoding,
            size,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum StorageType {
    Inline,
    File,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum PayloadFormat {
    Xml,
    Json,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum PayloadSchema {
    ISO20022,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum Encoding {
    #[serde(rename = "UTF-8")]
    Utf8,
    #[serde(rename = "UTF-16")]
    Utf16,
    #[serde(rename = "UTF-32")]
    Utf32,
    #[serde(rename = "ASCII")]
    Ascii,
}
