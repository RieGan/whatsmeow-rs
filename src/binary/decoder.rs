use crate::{
    binary::{Node, NodeContent, token::*},
    error::{Error, Result}
};
use std::collections::HashMap;

/// Decoder for WhatsApp binary protocol
pub struct BinaryDecoder<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> BinaryDecoder<'a> {
    /// Create a new decoder with the given data
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }
    
    /// Decode a node from the binary data
    pub fn decode(&mut self) -> Result<Node> {
        let tag = self.read_string_or_token()?;
        let attrs = self.read_attributes()?;
        
        let content = if self.pos >= self.data.len() {
            NodeContent::None
        } else {
            let content_type = self.peek_byte()?;
            if content_type == LIST_EMPTY {
                self.read_byte()?; // consume the LIST_EMPTY byte
                NodeContent::None
            } else if content_type >= LIST_8 && content_type <= JID_PAIR {
                self.read_children()?
            } else {
                self.read_binary_content()?
            }
        };
        
        Ok(Node {
            tag,
            attrs,
            content,
        })
    }
    
    /// Read string or token from the data
    fn read_string_or_token(&mut self) -> Result<String> {
        let byte = self.read_byte()?;
        
        match byte {
            0 => Ok(String::new()),
            1..=PACKED_MAX => {
                // Packed string
                let length = byte as usize;
                self.read_string(length)
            },
            128..=235 => {
                // Single-byte token
                if let Some(token) = get_single_token(byte) {
                    if !token.is_empty() {
                        Ok(token.to_string())
                    } else {
                        Err(Error::Protocol(format!("Invalid single-byte token: {}", byte)))
                    }
                } else {
                    Err(Error::Protocol(format!("Unknown single-byte token: {}", byte)))
                }
            },
            DICTIONARY_0..=DICTIONARY_3 => {
                // Double-byte token
                let dict = byte - DICTIONARY_0;
                let index = self.read_byte()?;
                if let Some(token) = get_double_token(dict, index) {
                    Ok(token.to_string())
                } else {
                    Err(Error::Protocol(format!("Unknown double-byte token: {}:{}", dict, index)))
                }
            },
            BINARY_8 => {
                let length = self.read_byte()? as usize;
                self.read_string(length)
            },
            BINARY_20 => {
                let length = self.read_int20()? as usize;
                self.read_string(length)
            },
            BINARY_32 => {
                let length = self.read_int32()? as usize;
                self.read_string(length)
            },
            _ => Err(Error::Protocol(format!("Unknown string/token type: {}", byte)))
        }
    }
    
    /// Read attributes from the data
    fn read_attributes(&mut self) -> Result<HashMap<String, String>> {
        let attr_count = self.read_list_size()?;
        let mut attrs = HashMap::new();
        
        for _ in 0..attr_count {
            let key = self.read_string_or_token()?;
            let value = self.read_string_or_token()?;
            attrs.insert(key, value);
        }
        
        Ok(attrs)
    }
    
    /// Read children nodes
    fn read_children(&mut self) -> Result<NodeContent> {
        let child_count = self.read_list_size()?;
        let mut children = Vec::new();
        
        for _ in 0..child_count {
            children.push(self.decode()?);
        }
        
        Ok(NodeContent::Children(children))
    }
    
    /// Read binary content
    fn read_binary_content(&mut self) -> Result<NodeContent> {
        let content_type = self.read_byte()?;
        
        match content_type {
            BINARY_8 => {
                let length = self.read_byte()? as usize;
                let data = self.read_bytes(length)?;
                Ok(NodeContent::Binary(data))
            },
            BINARY_20 => {
                let length = self.read_int20()? as usize;
                let data = self.read_bytes(length)?;
                Ok(NodeContent::Binary(data))
            },
            BINARY_32 => {
                let length = self.read_int32()? as usize;
                let data = self.read_bytes(length)?;
                Ok(NodeContent::Binary(data))
            },
            _ => {
                // Assume it's a string/text content
                self.pos -= 1; // Put back the byte
                let text = self.read_string_or_token()?;
                Ok(NodeContent::Text(text))
            }
        }
    }
    
    /// Read list size based on the current byte
    fn read_list_size(&mut self) -> Result<usize> {
        let size_type = self.read_byte()?;
        
        match size_type {
            LIST_EMPTY => Ok(0),
            LIST_8 => Ok(self.read_byte()? as usize),
            LIST_16 => Ok(self.read_int16()? as usize),
            _ => {
                self.pos -= 1; // Put back the byte
                Ok(0)
            }
        }
    }
    
    /// Read a byte from the data
    fn read_byte(&mut self) -> Result<u8> {
        if self.pos >= self.data.len() {
            return Err(Error::Protocol("Unexpected end of data".to_string()));
        }
        
        let byte = self.data[self.pos];
        self.pos += 1;
        Ok(byte)
    }
    
    /// Peek at the next byte without consuming it
    fn peek_byte(&self) -> Result<u8> {
        if self.pos >= self.data.len() {
            return Err(Error::Protocol("Unexpected end of data".to_string()));
        }
        
        Ok(self.data[self.pos])
    }
    
    /// Read multiple bytes from the data
    fn read_bytes(&mut self, count: usize) -> Result<Vec<u8>> {
        if self.pos + count > self.data.len() {
            return Err(Error::Protocol("Unexpected end of data".to_string()));
        }
        
        let bytes = self.data[self.pos..self.pos + count].to_vec();
        self.pos += count;
        Ok(bytes)
    }
    
    /// Read a string from the data
    fn read_string(&mut self, length: usize) -> Result<String> {
        let bytes = self.read_bytes(length)?;
        String::from_utf8(bytes)
            .map_err(|e| Error::Protocol(format!("Invalid UTF-8 string: {}", e)))
    }
    
    /// Read a 16-bit integer
    fn read_int16(&mut self) -> Result<u16> {
        let b1 = self.read_byte()? as u16;
        let b2 = self.read_byte()? as u16;
        Ok((b1 << 8) | b2)
    }
    
    /// Read a 20-bit integer (3 bytes)
    fn read_int20(&mut self) -> Result<u32> {
        let b1 = self.read_byte()? as u32;
        let b2 = self.read_byte()? as u32;
        let b3 = self.read_byte()? as u32;
        Ok((b1 << 16) | (b2 << 8) | b3)
    }
    
    /// Read a 32-bit integer
    fn read_int32(&mut self) -> Result<u32> {
        let b1 = self.read_byte()? as u32;
        let b2 = self.read_byte()? as u32;
        let b3 = self.read_byte()? as u32;
        let b4 = self.read_byte()? as u32;
        Ok((b1 << 24) | (b2 << 16) | (b3 << 8) | b4)
    }
}