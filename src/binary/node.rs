use std::collections::HashMap;

/// Represents a node in the WhatsApp binary protocol
#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    pub tag: String,
    pub attrs: HashMap<String, String>,
    pub content: NodeContent,
}

#[derive(Debug, Clone, PartialEq)]
pub enum NodeContent {
    None,
    Text(String),
    Binary(Vec<u8>),
    Children(Vec<Node>),
}

impl Node {
    /// Create a new node with tag
    pub fn new(tag: String) -> Self {
        Self {
            tag,
            attrs: HashMap::new(),
            content: NodeContent::None,
        }
    }
    
    /// Create node with attributes
    pub fn with_attrs(tag: String, attrs: HashMap<String, String>) -> Self {
        Self {
            tag,
            attrs,
            content: NodeContent::None,
        }
    }
    
    /// Set text content
    pub fn with_text(mut self, text: String) -> Self {
        self.content = NodeContent::Text(text);
        self
    }
    
    /// Set binary content
    pub fn with_binary(mut self, data: Vec<u8>) -> Self {
        self.content = NodeContent::Binary(data);
        self
    }
    
    /// Set children
    pub fn with_children(mut self, children: Vec<Node>) -> Self {
        self.content = NodeContent::Children(children);
        self
    }
    
    /// Add attribute
    pub fn attr(mut self, key: String, value: String) -> Self {
        self.attrs.insert(key, value);
        self
    }
    
    /// Get attribute value
    pub fn get_attr(&self, key: &str) -> Option<&String> {
        self.attrs.get(key)
    }
    
    /// Get children if content is children
    pub fn get_children(&self) -> Option<&Vec<Node>> {
        match &self.content {
            NodeContent::Children(children) => Some(children),
            _ => None,
        }
    }
    
    /// Get text content if content is text
    pub fn get_text(&self) -> Option<&String> {
        match &self.content {
            NodeContent::Text(text) => Some(text),
            _ => None,
        }
    }
    
    /// Get binary content if content is binary
    pub fn get_binary(&self) -> Option<&Vec<u8>> {
        match &self.content {
            NodeContent::Binary(data) => Some(data),
            _ => None,
        }
    }
    
    /// Find child by tag
    pub fn find_child(&self, tag: &str) -> Option<&Node> {
        if let Some(children) = self.get_children() {
            children.iter().find(|child| child.tag == tag)
        } else {
            None
        }
    }
}