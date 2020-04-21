use std::default::Default;
use std::io;

use html5ever::tendril::*;
use html5ever::tokenizer::BufferQueue;
use html5ever::tokenizer::{CharacterTokens, EndTag, NullCharacterToken, StartTag, TagToken, DoctypeToken, CommentToken, EOFToken};
use html5ever::tokenizer::{ParseError, Token, TokenSink, TokenSinkResult, Tokenizer, TokenizerOpts};

use std::io::Write;

#[derive(Debug,Default)]
pub struct PackedString {
    pub string: String,
    pub substrings: Vec<u32>,
}

impl PackedString {
    fn push_sub_str(&mut self, item: &str) {
        let a = self.string.len();
        let b = a + item.len();
        self.string.push_str(&item);
        self.substrings.push(a as u32);
        self.substrings.push(b as u32);
    }
}

#[derive(Debug,Default)]
pub struct BinaryHtmlEncoder {
    pub keys: PackedString,
    pub text: PackedString,
    pub attribute: PackedString,
    pub attribute_edges: Vec<u32>,
    pub element_edges: Vec<u32>,
    current_node: u32,
    current_attribute: u32,
    current_text_node: u32,
}

impl TokenSink for BinaryHtmlEncoder {
    type Handle = ();

    fn process_token(&mut self, token: Token, _ln: u64) -> TokenSinkResult<Self::Handle> {
        match token {
            // Do this after text
            DoctypeToken(doctype) => {
                self.keys.push_sub_str("!DOCTYPE");

                let parent = self.current_node;

                self.current_node += 1;

                self.element_edges.extend_from_slice(&[ 0, parent, self.current_node ]);

                if let Some(name) = doctype.name {
                    self.attribute.push_sub_str(&name);
                    self.attribute_edges.extend_from_slice(&[ self.current_node, self.current_attribute ]);
                };

                if let Some(public_id) = doctype.public_id {
                    self.attribute.push_sub_str(&public_id);
                    self.attribute_edges.extend_from_slice(&[ self.current_node, self.current_attribute + 1 ]);
                };

                if let Some(system_id) = doctype.system_id {
                    self.attribute.push_sub_str(&system_id);
                    self.attribute_edges.extend_from_slice(&[ self.current_node, self.current_attribute + 2 ]);
                };

                self.current_attribute += 3;
            },
            // Do this first
            CharacterTokens(text) => {
                self.text.push_sub_str(&text);
                self.element_edges.extend_from_slice(&[ 1, self.current_node, self.current_text_node ]);
                self.current_text_node += 1;
            },
            // Do this after doctype
            TagToken(tag) => {
                match tag.kind {
                    StartTag => {
                        self.keys.push_sub_str(&tag.name);

                        let parent = self.current_node;

                        self.current_node += 1;

                        self.element_edges.extend_from_slice(&[ 0, parent, self.current_node ]);

                        for (i, attribute) in tag.attrs.iter().enumerate() {
                            let key =
                                if !attribute.name.ns.is_empty() {
                                    [ &attribute.name.ns, ":", &attribute.name.local ].concat()
                                } else {
                                    attribute.name.local.to_string()
                                };
                            self.attribute.push_sub_str(&key);
                            self.attribute_edges.push(self.current_node);
                            self.attribute_edges.push(self.current_attribute + i as u32);
                        }

                        self.current_attribute += tag.attrs.len() as u32;
                    },
                    EndTag => {
                        self.current_node += 1;
                    },
                }
            },
            // Do the rest of the tokens after tags
            CommentToken(comment) => {

            },
            NullCharacterToken => {

            },
            EOFToken => {
                let mut header: Vec<u32> = Vec::with_capacity(9);

                header.push(32);
                header.push(header[0] + self.keys.string.len() as u32);
                header.push(header[1] + self.attribute.string.len() as u32);
                header.push(header[2] + self.text.string.len() as u32);
                header.push(header[3] + self.keys.substrings.len() as u32 * 4);
                header.push(header[4] + self.attribute.substrings.len() as u32 * 4);
                header.push(header[5] + self.text.substrings.len() as u32 * 4);
                header.push(header[6] + self.attribute_edges.len() as u32 * 4);
                header.push(header[7] + self.element_edges.len() as u32 * 4);

                let mut result: Vec<u8> = Vec::with_capacity(header[7] as usize);

                for offset in &header { result.extend(&offset.to_le_bytes()) }

                result.extend(self.keys.string.as_bytes());
                result.extend(self.attribute.string.as_bytes());
                result.extend(self.text.string.as_bytes());

                for index in &self.keys.substrings { result.extend(&index.to_le_bytes()) }
                for index in &self.attribute.substrings { result.extend(&index.to_le_bytes()) }
                for index in &self.text.substrings { result.extend(&index.to_le_bytes()) }

                for edge in &self.attribute_edges { result.extend(&edge.to_le_bytes()) }
                for edge in &self.element_edges { result.extend(&edge.to_le_bytes()) }

                let mut out = std::io::stdout();

                out.write_all(&result).unwrap();
            },
            ParseError(err) => {

            },
        }
        TokenSinkResult::Continue
    }
}

fn main() {
    let sink = BinaryHtmlEncoder::default();

    let mut chunk = ByteTendril::new();

    io::stdin().read_to_tendril(&mut chunk).unwrap();

    let mut input = BufferQueue::new();

    input.push_back(chunk.try_reinterpret().unwrap());

    let mut tok = Tokenizer::new(sink, TokenizerOpts::default());

    let _ = tok.feed(&mut input);

    assert!(input.is_empty());

    tok.end();
}
