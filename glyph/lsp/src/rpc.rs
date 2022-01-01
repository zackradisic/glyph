use anyhow::{anyhow, Error, Result};

use bytes::{Buf, BytesMut};
use combine::{easy, parser::combinator::AnySendPartialState, stream::PartialStream};
use common::Edit;
use jsonrpc_core::{
    serde_from_str, Notification as JsonNotification, Request as JsonRequest,
    Response as JsonResponse,
};
use lsp_types::{
    DidChangeTextDocumentParams, DidOpenTextDocumentParams, TextDocumentContentChangeEvent,
    TextDocumentIdentifier, TextDocumentItem, Url, VersionedTextDocumentIdentifier,
};
use macros::{make_notification, make_request};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;

use crate::parse;

const JSONRPC_VERSION: &str = "v2";

pub struct LanguageServerDecoder {
    state: AnySendPartialState,
}

impl LanguageServerDecoder {
    pub fn new() -> Self {
        Self {
            state: Default::default(),
        }
    }

    pub fn decode(&mut self, buf: &mut BytesMut) -> Result<Option<String>> {
        let (opt, removed_len) = combine::stream::decode(
            parse::decode_header(),
            &mut easy::Stream(PartialStream(&buf[..])),
            &mut self.state,
        )
        .map_err(|err| {
            let err = err
                .map_range(|r| {
                    std::str::from_utf8(r)
                        .ok()
                        .map_or_else(|| format!("{:?}", r), |s| s.to_string())
                })
                .map_position(|p| p.translate_position(&buf[..]));
            anyhow!("{}\nIn input: `{}`", err, std::str::from_utf8(buf).unwrap())
        })?;

        buf.advance(removed_len);

        match opt {
            None => Ok(None),
            Some(output) => {
                let value = String::from_utf8(output)?;
                Ok(Some(value))
            }
        }
    }
}

impl Default for LanguageServerDecoder {
    fn default() -> Self {
        Self::new()
    }
}

pub enum ServerResponse {
    Request(JsonRequest),
    Response(JsonResponse),
    Notification(JsonNotification),
}

impl LanguageServerDecoder {
    pub fn read_response(request_str: &str) -> Result<ServerResponse> {
        let val: Value = serde_from_str(request_str)?;
        match val {
            Value::Array(vals) => {
                todo!()
            }
            Value::Object(map) => {
                if map.contains_key("id") {
                    if map.contains_key("result") {
                        let res: JsonResponse = serde_json::from_value(Value::Object(map))?;
                        Ok(ServerResponse::Response(res))
                    } else {
                        let req: JsonRequest = serde_json::from_value(Value::Object(map))?;
                        Ok(ServerResponse::Request(req))
                    }
                } else {
                    let notif: JsonNotification = serde_json::from_value(Value::Object(map))?;
                    Ok(ServerResponse::Notification(notif))
                }
            }
            other => Err(anyhow::anyhow!("Unexpected json value: {:?}", other)),
        }
    }
}

#[derive(Clone, Copy)]
pub enum MessageKind {
    Request(Request),
    Notification(Notification),
    Unknown,
}

pub trait Message: std::fmt::Debug {
    fn to_bytes(&self) -> Result<Vec<u8>, Error>;

    // Return ID and request type, used for
    // keeping track of responses for deserialization
    fn request(&self) -> Option<Request>;

    fn set_id(&mut self, id: u8);
}

#[derive(Serialize, Debug)]
pub struct NotifMessage<'a, P> {
    jsonrpc: &'static str,
    method: &'a str,
    params: Option<P>,
    #[serde(skip_serializing)]
    pub kind: Notification,
}

impl<'a, P> Message for NotifMessage<'a, P>
where
    P: DeserializeOwned + Serialize + std::fmt::Debug,
{
    fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        serialize_with_content_length(self)
    }

    fn request(&self) -> Option<Request> {
        None
    }

    fn set_id(&mut self, _: u8) {}
}

impl<'a, P> NotifMessage<'a, P>
where
    P: DeserializeOwned + Serialize,
{
    pub fn new(method: &'a str, params: Option<P>, kind: Notification) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION,
            method,
            params,
            kind,
        }
    }
}

impl<'a> NotifMessage<'a, DidChangeTextDocumentParams> {
    pub fn from_edit(edit: &Edit, text_document: VersionedTextDocumentIdentifier) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION,
            method: "textDocument/didChange",
            params: Some(DidChangeTextDocumentParams {
                text_document,
                content_changes: vec![],
            }),
            kind: Notification::TextDocDidChange,
        }
    }
}

// pub fn edit_to_doc_change_event(edit: &Edit) -> TextDocumentContentChangeEvent {
//     TextDocumentContentChangeEvent {
//         range: ,
//         range_length: None,
//         text: edit.new_text.clone(),
//     }
// }

#[derive(Serialize, Debug)]
pub struct ReqMessage<'a, P> {
    jsonrpc: &'static str,
    method: &'a str,
    id: u8,
    params: P,
    #[serde(skip_serializing)]
    pub kind: Request,
}

impl<'a, P> Message for ReqMessage<'a, P>
where
    P: DeserializeOwned + Serialize + std::fmt::Debug,
{
    fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        serialize_with_content_length(self)
    }

    fn request(&self) -> Option<Request> {
        Some(self.kind)
    }

    fn set_id(&mut self, id: u8) {
        self.id = id;
    }
}

impl<'a, P> ReqMessage<'a, P>
where
    P: DeserializeOwned + Serialize,
{
    pub fn new(method: &'a str, params: P, kind: Request) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION,
            id: 1,
            method,
            params,
            kind,
        }
    }

    pub fn new_with_id(id: u8, method: &'a str, params: P, kind: Request) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION,
            id,
            method,
            params,
            kind,
        }
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        serialize_with_content_length(self)
    }
}

pub fn serialize_with_content_length<P: Serialize>(val: &P) -> Result<Vec<u8>, Error> {
    let s = serde_json::to_string(&val)?;
    Ok(
        format!("Content-Length: {}\r\n\r\n{}", s.as_bytes().len(), s)
            .as_bytes()
            .to_vec(),
    )
}

make_request!(Initialize, TextDocDefinition);
make_notification!(
    Initialized,
    TextDocDidOpen,
    TextDocDidClose,
    TextDocDidChange
);

pub fn text_doc_did_open<'a>(
    uri: Url,
    language_id: String,
    version: i32,
    text: String,
) -> NotifMessage<'a, DidOpenTextDocumentParams> {
    NotifMessage::new(
        "textDocument/didOpen",
        Some(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri,
                language_id,
                version,
                text,
            },
        }),
        Notification::TextDocDidOpen,
    )
}
