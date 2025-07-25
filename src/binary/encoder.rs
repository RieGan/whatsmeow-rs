use crate::{binary::{Node, NodeContent}, error::Result};

/// Encoder for WhatsApp binary protocol
pub struct BinaryEncoder {
    data: Vec<u8>,
}

impl BinaryEncoder {
    /// Create a new encoder
    pub fn new() -> Self {
        Self { data: Vec::new() }
    }
    
    /// Encode a node to binary data
    pub fn encode(&mut self, node: &Node) -> Result<Vec<u8>> {
        self.data.clear();
        
        // TODO: Implement actual WhatsApp binary protocol encoding
        // This is a placeholder implementation
        
        self.write_string(&node.tag)?;
        self.write_attributes(&node.attrs)?;
        self.write_content(&node.content)?;
        
        Ok(self.data.clone())
    }
    
    /// Write a string to the data
    fn write_string(&mut self, s: &str) -> Result<()> {
        let bytes = s.as_bytes();
        self.write_varint(bytes.len() as u64)?;
        self.data.extend_from_slice(bytes);
        Ok(())
    }
    
    /// Write attributes to the data
    fn write_attributes(&mut self, attrs: &std::collections::HashMap<String, String>) -> Result<()> {
        self.write_varint(attrs.len() as u64)?;
        
        for (key, value) in attrs {
            self.write_string(key)?;
            self.write_string(value)?;
        }
        
        Ok(())
    }
    
    /// Write node content to the data
    fn write_content(&mut self, content: &NodeContent) -> Result<()> {
        match content {
            NodeContent::None => {
                self.data.push(0);
            },
            NodeContent::Text(text) => {
                self.data.push(1);
                self.write_string(text)?;
            },
            NodeContent::Binary(data) => {
                self.data.push(2);
                self.write_varint(data.len() as u64)?;
                self.data.extend_from_slice(data);
            },
            NodeContent::Children(children) => {
                self.data.push(3);
                self.write_varint(children.len() as u64)?;
                for child in children {
                    let child_data = BinaryEncoder::new().encode(child)?;
                    self.data.extend_from_slice(&child_data);
                }
            },
        }
        
        Ok(())
    }
    
    /// Write a varint to the data
    fn write_varint(&mut self, mut value: u64) -> Result<()> {
        while value >= 0x80 {
            self.data.push((value & 0x7F | 0x80) as u8);
            value >>= 7;
        }
        self.data.push(value as u8);
        Ok(())
    }
}