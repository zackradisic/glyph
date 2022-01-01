use anyhow::Result;
use colored::Colorize;
use common::Edit;
use std::{
    collections::HashMap,
    ffi::OsStr,
    io::Write,
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
    rc::Rc,
    sync::{
        mpsc::{self, Receiver, Sender},
        Arc, RwLock,
    },
    thread::{self},
    time::Duration,
};

use bytes::BytesMut;
use jsonrpc_core::{
    Failure, Notification as JsonNotification, Output, Params, Response as JsonResponse, Success,
    Value,
};
use lsp_types::{
    ClientCapabilities, Diagnostic, InitializeParams, InitializeResult, InitializedParams,
    PublishDiagnosticsParams, ServerCapabilities, TextDocumentContentChangeEvent,
    TextDocumentIdentifier, TextEdit, Url, VersionedTextDocumentIdentifier,
    WorkspaceClientCapabilities,
};
use serde::de::DeserializeOwned;

use crate::{
    nonblock::NonBlockingReader, LanguageServerDecoder, Message, NotifMessage, Notification,
    ReqMessage, Request, ServerResponse,
};

pub enum Either<L, R> {
    Left(L),
    Right(R),
}

#[derive(Clone)]
pub struct LspSender {
    // TODO: Get rid of dynamic dispatch
    tx: Sender<Box<dyn Message + Send>>,
    debounce_tx: Sender<Box<dyn Message + Send>>,
}

impl LspSender {
    fn wrap(tx: Sender<Box<dyn Message + Send>>) -> Self {
        let (debounce_tx, debounce_rx) = mpsc::channel();

        thread::spawn(move || Self::debounce_loop(debounce_rx));

        Self { tx, debounce_tx }
    }

    fn debounce_loop(rx: Receiver<Box<dyn Message + Send>>) {
        for msg in rx {}
    }

    pub fn send_message(&self, data: Box<dyn Message + Send>) {
        #[cfg(debug_assertions)]
        println!("Sending message: {:?}", data);
        self.tx.send(data).unwrap()
    }

    pub fn send_edit(
        &self,
        edit: TextDocumentContentChangeEvent,
        doc: Arc<RwLock<VersionedTextDocumentIdentifier>>,
    ) {
    }

    pub fn send_edit_debounce(
        &self,
        edit: TextDocumentContentChangeEvent,
        doc: Arc<RwLock<VersionedTextDocumentIdentifier>>,
    ) {
    }

    pub fn send_message_debounce(&self, data: Box<dyn Message + Send>) {}
}

#[derive(Debug)]
pub struct Diagnostics {
    pub diagnostics: Vec<Diagnostic>,
    pub clock: u64,
}

impl Diagnostics {
    pub fn new() -> Self {
        Self {
            diagnostics: Vec::new(),
            clock: 1,
        }
    }

    pub fn update(&mut self, diagnostics: Vec<Diagnostic>) {
        self.diagnostics = diagnostics;
        self.clock += 1;
    }
}

impl Default for Diagnostics {
    fn default() -> Self {
        Self::new()
    }
}

pub struct Client {
    diagnostics: Arc<RwLock<Diagnostics>>,
    server_capabilities: Rc<ServerCapabilities>,

    // Internal stuff
    tx: LspSender,
    write_thread_id: u64,
    read_thread_id: u64,
    child: Child,
}

impl Drop for Client {
    fn drop(&mut self) {
        unsafe {
            libc::pthread_kill(self.write_thread_id as usize, libc::SIGINT);
            libc::pthread_kill(self.read_thread_id as usize, libc::SIGINT);
        }
        self.child.kill().unwrap()
    }
}

impl Client {
    pub fn new<T: AsRef<OsStr>>(cmd_path: T, cwd: &str) -> Self {
        let diagnostics = Arc::new(RwLock::new(Diagnostics::new()));

        let mut cmd = Command::new(cmd_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .current_dir(cwd)
            .spawn()
            .unwrap();

        let msg = Box::new(ReqMessage::new(
            "initialize",
            Self::initialize_params(cmd.id(), cwd),
            Request::Initialize,
        ));

        let (tx, rx) = mpsc::channel::<Box<dyn Message + Send>>();
        let tx = LspSender::wrap(tx);
        let stdin = cmd.stdin.take().unwrap();
        let stdout = NonBlockingReader::from_fd(cmd.stdout.take().unwrap()).unwrap();

        let server_capabilities = Arc::new(RwLock::new(None));

        let inner = Inner {
            server_capabilities: server_capabilities.clone(),
            diagnostics: diagnostics.clone(),
            request_ids: Arc::new(RwLock::new(HashMap::new())),
            req_id_counter: Default::default(),
            tx: tx.clone(),
        };
        let inner_clone = inner.clone();

        let write_thread_id = thread::spawn(move || inner_clone.stdin(rx, stdin))
            .thread()
            .id()
            .as_u64()
            .get();
        let read_thread_id = thread::spawn(move || inner.stdout(stdout))
            .thread()
            .id()
            .as_u64()
            .get();

        tx.send_message(msg);

        Self {
            server_capabilities: Rc::new(Self::wait_for_capabilities(server_capabilities)),
            diagnostics,

            tx,
            write_thread_id,
            read_thread_id,
            child: cmd,
        }
    }

    pub fn send_message(&self, data: Box<dyn Message + Send>) {
        self.tx.send_message(data)
    }

    fn initialize_params(process_id: u32, cwd: &str) -> InitializeParams {
        InitializeParams {
            process_id: Some(process_id),
            root_uri: Some(Url::parse(&format!("file://{}", cwd)).unwrap()),
            initialization_options: None,
            capabilities: ClientCapabilities {
                workspace: Some(WorkspaceClientCapabilities {
                    apply_edit: Some(true),
                    workspace_edit: None,
                    did_change_configuration: None,
                    did_change_watched_files: None,
                    symbol: None,
                    execute_command: None,
                    workspace_folders: Some(true),
                    configuration: Some(true),
                    semantic_tokens: None,
                    code_lens: None,
                    file_operations: None,
                }),
                text_document: None,
                window: None,
                general: None,
                experimental: None,
            },
            trace: None,
            workspace_folders: None,
            client_info: None,
            locale: None,
            root_path: None,
        }
    }

    /// Blocks the current thread until server capabilities are set and returns
    /// a copy of them
    fn wait_for_capabilities(
        capabilities: Arc<RwLock<Option<ServerCapabilities>>>,
    ) -> ServerCapabilities {
        loop {
            println!("Waiting for capabilities...");
            let capabilities = { capabilities.read().unwrap().clone() };
            if let Some(capabilities) = capabilities {
                println!("Got capabilities!");
                return capabilities;
            }
            std::thread::sleep(Duration::from_millis(100))
        }
    }

    pub fn capabilities(&self) -> &Rc<ServerCapabilities> {
        &self.server_capabilities
    }

    pub fn diagnostics(&self) -> &Arc<RwLock<Diagnostics>> {
        &self.diagnostics
    }

    pub fn sender(&self) -> &LspSender {
        &self.tx
    }
}

/// This struct is cloned and given to the reader and writer
/// threads respectively
#[derive(Clone)]
struct Inner {
    server_capabilities: Arc<RwLock<Option<ServerCapabilities>>>,
    diagnostics: Arc<RwLock<Diagnostics>>,
    request_ids: Arc<RwLock<HashMap<u16, Request>>>,
    req_id_counter: Arc<RwLock<u16>>,
    tx: LspSender,
}

// Functions that execute in threads
impl Inner {
    fn stdin(&self, rx: Receiver<Box<dyn Message + Send>>, mut stdin: ChildStdin) {
        // Write all messages coming from rx
        for mut msg in rx {
            if let Some(req) = msg.request() {
                let mut req_ids = self.request_ids.write().unwrap();
                let mut req_id_counter = self.req_id_counter.write().unwrap();
                *req_id_counter += 1;
                msg.set_id(*req_id_counter as u8);
                req_ids.insert(*req_id_counter, req);
            }
            stdin.write_all(&msg.to_bytes().unwrap()).unwrap();
        }
    }

    /// Reads LSP JSON RPC messages from stdout, dispatching
    /// on the method kind.
    fn stdout(&self, mut stdout: NonBlockingReader<ChildStdout>) {
        let mut decoder = LanguageServerDecoder::new();
        let mut buf = BytesMut::new();
        let mut read: usize;

        loop {
            read = match stdout.read_available(&mut buf) {
                Err(e) => panic!("Error from stdout: {:?}", e),
                Ok(r) => r,
            };

            // 0 may indicate EOF or simply that there is no data
            // ready for reading yet
            if read == 0 && stdout.is_eof() {
                panic!("Got unexpected EOF from language server");
            }

            if buf.len() > 5 {
                let title = String::from_utf8(buf.to_vec()).unwrap();
                println!("{}", format!("F: {}", title).blue());
            }

            match decoder.decode(&mut buf) {
                Ok(Some(s)) => match LanguageServerDecoder::read_response(&s) {
                    Ok(ServerResponse::Response(res)) => match res {
                        JsonResponse::Single(output) => self.handle_response(output),
                        JsonResponse::Batch(outputs) => outputs
                            .into_iter()
                            .for_each(|output| self.handle_response(output)),
                    },
                    Ok(ServerResponse::Notification(JsonNotification {
                        method, params, ..
                    })) => self.handle_notification(method, params),
                    Ok(ServerResponse::Request(_)) => {
                        todo!()
                    }
                    Err(e) => {
                        panic!("Invalid JSON RPC message: {:?} {}", e, s.blue())
                    }
                },
                Ok(None) => {}
                Err(e) => panic!("Error from decoder: {:?}", e),
            }
        }
    }

    fn handle_response(&self, output: Output) {
        match output {
            Output::Success(Success {
                result,
                id: jsonrpc_core::Id::Num(id),
                ..
            }) => {
                if id > u16::MAX as u64 {
                    panic!("Invalid id: {}", id);
                }
                let req = {
                    let request_ids = self.request_ids.read().unwrap();
                    request_ids.get(&(id as u16)).cloned()
                };
                if let Some(req) = req {
                    self.handle_successful_response(result, req)
                } else {
                    eprintln!("Request response with id ({}) has no mapping", id);
                }
            }
            Output::Failure(Failure { id, error, .. }) => {
                eprintln!("Error: {:?} {:?}", id, error)
            }
            _ => eprintln!("Invalid output: {:?}", output),
        }
    }
}

// Request response handlers
impl Inner {
    fn handle_successful_response(&self, result: serde_json::Value, request: Request) {
        match request {
            Request::Initialize => self.initialized(serde_json::from_value(result).unwrap()),
            Request::TextDocDefinition => todo!(),
        }
    }

    fn initialized(&self, result: InitializeResult) {
        // TODO: Capture name and show it on the status line
        let _ = result
            .server_info
            .map(|info| info.name)
            .unwrap_or_else(|| "Rust".into());

        {
            let mut server_capabilities = self.server_capabilities.write().unwrap();
            let _ = server_capabilities.insert(result.capabilities);
        }

        // Respond with acknowledgement
        let msg = Box::new(NotifMessage::new(
            "initialized",
            Some(InitializedParams {}),
            Notification::Initialized,
        ));

        self.tx.send_message(msg);
    }
}

// Notifications
impl Inner {
    fn handle_notification(&self, method: String, params: Params) {
        match method.as_str() {
            "textDocument/publishDiagnostics" => {
                self.handle_publish_diagnostics(params).unwrap();
            }
            o => {
                println!("Unknown notification: {:?}", o);
            }
        }
    }
    fn handle_publish_diagnostics(&self, params: Params) -> Result<()> {
        let params: PublishDiagnosticsParams = Self::from_value(params)?;

        let mut diagnostics = self.diagnostics.write().unwrap();
        diagnostics.update(params.diagnostics);

        Ok(())
    }
}

// Utility
impl Inner {
    fn from_value<T: DeserializeOwned>(p: Params) -> Result<T> {
        let res = match p {
            Params::Map(map) => serde_json::from_value::<T>(Value::Object(map)),
            Params::Array(val) => serde_json::from_value::<T>(Value::Array(val)),
            _ => todo!(),
        };

        res.map_err(|e| anyhow::anyhow!("Serde error: {:?}", e))
    }
}

pub fn transmute_u16s(bytes: Vec<u16>) -> Vec<u8> {
    // This operation is sound because u16 = 2 u8s
    // so there should be no alignment issues.
    let ret = unsafe {
        Vec::<u8>::from_raw_parts(bytes.as_ptr() as *mut u8, bytes.len() * 2, bytes.capacity())
    };

    bytes.leak();

    ret
}

#[cfg(test)]
mod test {
    use std::time::Duration;

    use lsp_types::{DidOpenTextDocumentParams, TextDocumentItem, Url};

    use crate::{transmute_u16s, Client};

    #[test]
    fn it_works() {
        let client = Client::new(
            "/usr/local/bin/rust-analyzer",
            "/Users/zackradisic/Desktop/Code/lsp-test-workspace",
        );
        let _tx = &client.tx;

        let _f = DidOpenTextDocumentParams {
            text_document: TextDocumentItem::new(
                Url::parse("file://main.rs").unwrap(),
                "rust".into(),
                0,
                "fn main() { println!(\"HELLO!\"); }".into(),
            ),
        };
        // let notif = Message::new("textDocument/didOpen", f);

        // let json_str = serde_json::to_string(&notif).unwrap();
        // println!("JSON: {}", json_str);
        // tx.send(json_str.into_bytes()).unwrap();
        std::thread::sleep(Duration::from_millis(3000));
    }

    #[test]
    fn transmute_u16s_works() {
        fn run(src: Vec<u16>, expect: Vec<u8>) {
            let out = transmute_u16s(src.clone());
            assert_eq!(out, expect);
            assert_eq!(
                src,
                out.chunks(2)
                    .into_iter()
                    .map(|a| u16::from_ne_bytes([a[0], a[1]]))
                    .collect::<Vec<u16>>()
            )
        }

        run(
            vec![1, 2, 3],
            vec![
                1u16 as u8,
                (1u16 >> 8) as u8,
                2u16 as u8,
                (2u16 >> 8) as u8,
                3u16 as u8,
                (3u16 >> 8) as u8,
            ],
        );

        run(
            vec![69, 420, 4200],
            vec![
                69_u8,
                (69 >> 8) as u8,
                (420 & 0b11111111) as u8,
                (420 >> 8) as u8,
                (4200 & 0b11111111) as u8,
                (4200 >> 8) as u8,
            ],
        );
    }
}
