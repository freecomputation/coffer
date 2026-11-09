//! umiak-server: serves the Shelf over loopback HTTP.
//!
//! Routes:
//!   GET /                    shelf home: book list + search box
//!   GET /book/<id>           redirect to the book's main entry
//!   GET /book/<id>/<path>    one item out of a book, with its MIME type
//!   GET /search?q=<query>    one search across every book aboard
//!   GET /api/books           the shelf as JSON
//!   GET /api/search?q=       search results as JSON
//!
//! The webview chrome loads this server; nothing in Umiak renders around
//! it. That keeps the engine borrowed and swappable (design determination 1).

use std::io::Cursor;
use std::path::PathBuf;
use std::sync::Arc;

use tiny_http::{Header, Response, Server};
use umiak_core::Shelf;

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn json_esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

fn url_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => {
                let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or("");
                match u8::from_str_radix(hex, 16) {
                    Ok(v) => {
                        out.push(v);
                        i += 3;
                    }
                    Err(_) => {
                        out.push(bytes[i]);
                        i += 1;
                    }
                }
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn query_param(url: &str, key: &str) -> Option<String> {
    let q = url.split_once('?')?.1;
    for pair in q.split('&') {
        if let Some((k, v)) = pair.split_once('=') {
            if k == key {
                return Some(url_decode(v));
            }
        } else if pair == key {
            return Some(String::new());
        }
    }
    None
}

fn html_response(body: String) -> Response<Cursor<Vec<u8>>> {
    Response::from_string(body).with_header(
        Header::from_bytes(&b"Content-Type"[..], &b"text/html; charset=utf-8"[..]).unwrap(),
    )
}

fn json_response(body: String) -> Response<Cursor<Vec<u8>>> {
    Response::from_string(body)
        .with_header(Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap())
}

fn home(shelf: &Shelf) -> String {
    let mut items = String::new();
    for b in &shelf.books {
        items.push_str(&format!(
            "<li><a href=\"/book/{}\">{}</a> <span class=\"meta\">{} articles, {}</span></li>\n",
            b.id,
            esc(&b.title),
            b.article_count,
            esc(&b.file_name)
        ));
    }
    if shelf.books.is_empty() {
        items = "<li>No books aboard yet. Add .zim files to the shelf directory.</li>".to_string();
    }
    format!(
        "<!DOCTYPE html>\n<html><head><meta charset=\"utf-8\"><title>Umiak</title></head>\n\
         <body><h1>Umiak</h1>\n\
         <form action=\"/search\" method=\"get\"><input name=\"q\" size=\"40\" \
         placeholder=\"Search everything aboard\"> <input type=\"submit\" value=\"Search\"></form>\n\
         <h2>Aboard</h2>\n<ul>\n{}</ul>\n</body></html>",
        items
    )
}

struct Hit {
    book: usize,
    path: String,
    title: String,
    score: f64,
}

/// One search across every book aboard. Each book answers in its own
/// ranking (full text where it has an index, title suggestions where it
/// does not); the lists are merged by reciprocal rank fusion so a book's
/// best answer outranks another book's twentieth. Rank inside the book
/// is the only signal the books share, which is exactly what RRF needs.
fn federated_search(shelf: &Shelf, query: &str) -> Vec<Hit> {
    const RRF_K: f64 = 60.0;
    let mut hits: Vec<Hit> = Vec::new();
    for b in &shelf.books {
        let results = match b.search(query, 20) {
            Ok(h) => h,
            Err(_) => b.suggest(query, 20).unwrap_or_default(),
        };
        for (rank, (path, title)) in results.into_iter().enumerate() {
            hits.push(Hit {
                book: b.id,
                path,
                title,
                score: 1.0 / (RRF_K + rank as f64 + 1.0),
            });
        }
    }
    hits.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.book.cmp(&b.book))
    });
    hits.truncate(50);
    hits
}

fn search_page(shelf: &Shelf, query: &str) -> String {
    let hits = federated_search(shelf, query);
    let mut body = String::new();
    if hits.is_empty() {
        body = "<p>Nothing aboard matches that.</p>".to_string();
    } else {
        body.push_str("<ul>\n");
        for h in &hits {
            let book_title = shelf
                .book(h.book)
                .map(|b| b.title.as_str())
                .unwrap_or_default();
            body.push_str(&format!(
                "<li><a href=\"/book/{}/{}\">{}</a> <span class=\"meta\">in {}</span></li>\n",
                h.book,
                h.path,
                esc(if h.title.is_empty() { &h.path } else { &h.title }),
                esc(book_title)
            ));
        }
        body.push_str("</ul>\n");
    }
    format!(
        "<!DOCTYPE html>\n<html><head><meta charset=\"utf-8\"><title>Search</title></head>\n\
         <body><h1>Search: {}</h1>\n{}\n<p><a href=\"/\">Shelf</a></p></body></html>",
        esc(query),
        body
    )
}

fn books_json(shelf: &Shelf) -> String {
    let mut out = String::from("{\"books\":[");
    for (i, b) in shelf.books.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str(&format!(
            "{{\"id\":{},\"file\":\"{}\",\"title\":\"{}\",\"language\":\"{}\",\"entries\":{},\"articles\":{},\"main_path\":\"{}\"}}",
            b.id,
            json_esc(&b.file_name),
            json_esc(&b.title),
            json_esc(&b.language),
            b.entry_count,
            b.article_count,
            json_esc(&b.main_path)
        ));
    }
    out.push_str("]}");
    out
}

fn search_json(shelf: &Shelf, query: &str) -> String {
    let hits = federated_search(shelf, query);
    let mut out = String::from("{\"results\":[");
    for (i, h) in hits.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        let book_title = shelf
            .book(h.book)
            .map(|b| b.title.as_str())
            .unwrap_or_default();
        out.push_str(&format!(
            "{{\"book\":{},\"book_title\":\"{}\",\"path\":\"{}\",\"title\":\"{}\"}}",
            h.book,
            json_esc(book_title),
            json_esc(&h.path),
            json_esc(&h.title)
        ));
    }
    out.push_str("]}");
    out
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut shelf_dir = std::env::var("UMIAK_SHELF").unwrap_or_else(|_| "shelf".to_string());
    let mut port: u16 = 27331;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--shelf" if i + 1 < args.len() => {
                shelf_dir = args[i + 1].clone();
                i += 2;
            }
            "--port" if i + 1 < args.len() => {
                port = args[i + 1].parse().unwrap_or(port);
                i += 2;
            }
            _ => i += 1,
        }
    }

    let shelf = Arc::new(Shelf::open_dir(&PathBuf::from(&shelf_dir)));
    println!(
        "umiak-server: {} book(s) aboard from {}",
        shelf.books.len(),
        shelf_dir
    );

    let server = Server::http(("127.0.0.1", port)).expect("bind loopback");
    println!("umiak-server: listening on http://127.0.0.1:{}/", port);

    for request in server.incoming_requests() {
        let url = request.url().to_string();
        let path = url.split('?').next().unwrap_or("/").to_string();

        let response = if path == "/" {
            html_response(home(&shelf))
        } else if path == "/api/books" {
            json_response(books_json(&shelf))
        } else if path == "/search" {
            let q = query_param(&url, "q").unwrap_or_default();
            html_response(search_page(&shelf, &q))
        } else if path == "/api/search" {
            let q = query_param(&url, "q").unwrap_or_default();
            json_response(search_json(&shelf, &q))
        } else if let Some(rest) = path.strip_prefix("/book/") {
            let (id_part, item_path) = match rest.split_once('/') {
                Some((a, b)) => (a, b.to_string()),
                None => (rest, String::new()),
            };
            match id_part.parse::<usize>().ok().and_then(|id| shelf.book(id)) {
                Some(book) => {
                    if item_path.is_empty() {
                        let location = format!("/book/{}/{}", book.id, book.main_path);
                        Response::from_data(Vec::new())
                            .with_status_code(302)
                            .with_header(
                                Header::from_bytes(&b"Location"[..], location.as_bytes())
                                    .unwrap(),
                            )
                    } else {
                        let decoded = url_decode(&item_path);
                        match book.get(&decoded) {
                            Ok(Some((mime, bytes))) => Response::from_data(bytes)
                                .with_header(
                                    Header::from_bytes(&b"Content-Type"[..], mime.as_bytes())
                                        .unwrap(),
                                ),
                            Ok(None) => Response::from_string("not aboard\n").with_status_code(404),
                            Err(e) => Response::from_string(format!("error: {}\n", e))
                                .with_status_code(500),
                        }
                    }
                }
                None => Response::from_string("no such book\n").with_status_code(404),
            }
        } else {
            Response::from_string("not found\n").with_status_code(404)
        };

        let _ = request.respond(response);
    }
}
