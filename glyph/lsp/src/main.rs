use std::{fs, path::PathBuf, time::Duration};

use lsp::{Client, Either, Message, MessageKind, NotifMessage, Notification, ReqMessage};
use lsp_types::{DidOpenTextDocumentParams, TextDocumentItem, Url};

fn main() {
    println!("HI");
    let srcdir = PathBuf::from("/Users/zackradisic/Desktop/Code/lsp-test-workspace");
    println!("{:?}", fs::canonicalize(&srcdir));
    let client = Client::new(
        "/usr/local/bin/rust-analyzer",
        "/Users/zackradisic/Desktop/Code/lsp-test-workspace",
    );

    println!("{:?}", client.diagnostics());

    println!(
        "URI: {}",
        Url::parse("file://Users/zackradisic/Desktop/Code/lsp-test-workspace").unwrap()
    );
    let f = DidOpenTextDocumentParams {
        text_document: TextDocumentItem::new(
            Url::parse("file:///Users/zackradisic/Desktop/Code/lsp-test-workspace/src/lib.rs")
                .unwrap(),
            "rust".into(),
            0,
            "fn main() { printasfasfln!(\"HELLO!\") }".into(),
        ),
    };
    let notif = NotifMessage::new(
        "textDocument/didOpen",
        Some(f),
        Notification::TextDocDidOpen,
    );

    std::thread::sleep(Duration::from_millis(3000));
    client.send_message(Either::Right(&notif));

    std::thread::sleep(Duration::from_millis(10000));
}
