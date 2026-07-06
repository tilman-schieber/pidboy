//! `pidc serve`: local file bridge for the browser editor.
//!
//! The server never compiles anything — compilation runs client-side in
//! pidc-wasm. It hands out the static editor bundle plus the `.pid` file,
//! and writes edits back to disk on POST /save.

use tiny_http::{Header, Method, Response, Server};

const INDEX_HTML: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/index.html"));
const EDITOR_JS: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/editor.js"));
const WASM_JS: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/pkg_pidc_wasm.js"));
const WASM_BIN: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/pkg_pidc_wasm_bg.wasm"));

fn content_type(value: &str) -> Header {
    Header::from_bytes(&b"Content-Type"[..], value.as_bytes()).unwrap()
}

pub fn serve(input: &str, port: u16) -> i32 {
    if WASM_BIN.is_empty() {
        eprintln!("error: editor assets not built; run scripts/build-editor.sh and rebuild pidc");
        return 1;
    }
    if let Err(e) = std::fs::metadata(input) {
        eprintln!("error: cannot read `{}`: {}", input, e);
        return 1;
    }

    let addr = format!("127.0.0.1:{}", port);
    let server = match Server::http(&addr) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: cannot bind {}: {}", addr, e);
            return 1;
        }
    };
    eprintln!("serving http://{} (editing {})", addr, input);
    eprintln!("press Ctrl-C to stop");

    for mut request in server.incoming_requests() {
        let method = request.method().clone();
        let url = request.url().to_string();
        let path = url.split('?').next().unwrap_or("/");
        let response = match (&method, path) {
            (Method::Get, "/") => {
                Response::from_data(INDEX_HTML).with_header(content_type("text/html; charset=utf-8"))
            }
            (Method::Get, "/editor.js") => {
                Response::from_data(EDITOR_JS).with_header(content_type("text/javascript"))
            }
            (Method::Get, "/pkg/pidc_wasm.js") => {
                Response::from_data(WASM_JS).with_header(content_type("text/javascript"))
            }
            (Method::Get, "/pkg/pidc_wasm_bg.wasm") => {
                Response::from_data(WASM_BIN).with_header(content_type("application/wasm"))
            }
            (Method::Get, "/meta") => {
                let json = format!("{{\"filename\":{}}}", json_string(input));
                Response::from_data(json.into_bytes())
                    .with_header(content_type("application/json"))
            }
            // Read fresh on every request so external edits show up on reload.
            (Method::Get, "/source") => match std::fs::read(input) {
                Ok(bytes) => Response::from_data(bytes)
                    .with_header(content_type("text/plain; charset=utf-8")),
                Err(e) => Response::from_data(format!("cannot read {}: {}", input, e).into_bytes())
                    .with_status_code(500),
            },
            (Method::Post, "/save") => {
                let mut body = Vec::new();
                match request.as_reader().read_to_end(&mut body) {
                    Ok(_) => match std::fs::write(input, &body) {
                        Ok(_) => Response::from_data(Vec::new()).with_status_code(204),
                        Err(e) => Response::from_data(
                            format!("cannot write {}: {}", input, e).into_bytes(),
                        )
                        .with_status_code(500),
                    },
                    Err(e) => Response::from_data(format!("bad request body: {}", e).into_bytes())
                        .with_status_code(400),
                }
            }
            _ => Response::from_data(b"not found".to_vec()).with_status_code(404),
        };
        let _ = request.respond(response);
    }
    0
}

fn json_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}
