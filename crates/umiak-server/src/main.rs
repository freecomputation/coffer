//! umiak-server: serves the Shelf over loopback HTTP.
//!
//! Routes:
//!   GET /                    the chrome: nav bar, address field, tabs
//!   GET /shelf               shelf home: book list + search box
//!   GET /book/<id>           redirect to the book's main entry
//!   GET /book/<id>/<path>    one item out of a book, with its MIME type
//!   GET /search?q=<query>    one search across every book aboard
//!   GET /catalog?q=&lang=&c= the Kiwix OPDS catalog, organized by
//!                            category and publisher, when a wire exists
//!   GET /acquire?url=&name=  start fetching a book onto the shelf
//!   GET /acquire-progress    watch the fetch come down
//!   GET /api/acquire-status  the fetch as JSON
//!   GET /api/books           the shelf as JSON
//!   GET /api/search?q=       search results as JSON
//!
//! The webview window loads this server; nothing in Umiak renders around
//! it. That keeps the engine borrowed and swappable (determination 1).
//! Requests are handled on threads: a long download never blocks reading.

use std::io::{Cursor, Read};
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

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

fn url_encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
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

/// The chrome: one page that carries navigation for every surface.
/// Plain script, no build step, nothing fetched. Tabs are saved states:
/// one live view, and switching a tab swaps the address it shows.
const CHROME: &str = r##"<!DOCTYPE html>
<html><head><meta charset="utf-8"><title>Umiak</title>
<style>
html,body{margin:0;height:100%;font-family:system-ui,sans-serif;background:#fff;color:#111}
#bar{display:flex;gap:6px;align-items:center;padding:6px 8px;background:#eee;border-bottom:1px solid #ccc}
#bar button{font-size:15px;padding:2px 9px;cursor:pointer}
#addr{flex:1;font-size:15px;padding:4px 8px}
#tabs{display:flex;gap:4px;align-items:center;padding:4px 8px;background:#f7f7f7;border-bottom:1px solid #ddd;flex-wrap:wrap}
.tab{font-size:13px;padding:3px 8px;cursor:pointer;border:1px solid #ccc;background:#fff;max-width:220px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
.tab.on{background:#dde7ff;border-color:#88a}
.tabx{font-size:12px;padding:3px 6px;cursor:pointer;border:1px solid #ddd;background:#fff}
#view{width:100%;height:calc(100% - 84px);border:0;display:block}
</style></head>
<body>
<div id="bar">
<button id="back" title="Back">&#8249;</button>
<button id="fwd" title="Forward">&#8250;</button>
<button id="reload" title="Reload">&#8635;</button>
<input id="addr" placeholder="Search the Shelf, or a path like /book/0">
<button id="go">Go</button>
<button id="shelfbtn">Shelf</button>
<button id="catbtn">Catalog</button>
</div>
<div id="tabs"></div>
<iframe id="view" src="/shelf"></iframe>
<script>
var tabs=[{url:'/shelf',title:'Shelf'}], active=0;
var view=document.getElementById('view'), addr=document.getElementById('addr');
function renderTabs(){
  var t=document.getElementById('tabs'); t.innerHTML='';
  tabs.forEach(function(tab,i){
    var b=document.createElement('button');
    b.textContent=tab.title||tab.url; b.className='tab'+(i===active?' on':'');
    b.onclick=function(){switchTab(i)};
    var x=document.createElement('button');
    x.textContent='\u00d7'; x.className='tabx'; x.title='Close tab';
    x.onclick=function(ev){ev.stopPropagation(); closeTab(i)};
    t.appendChild(b); t.appendChild(x);
  });
  var n=document.createElement('button');
  n.textContent='+'; n.className='tabx'; n.title='New tab';
  n.onclick=function(){newTab()};
  t.appendChild(n);
}
function current(){
  try { return view.contentWindow.location.pathname + view.contentWindow.location.search; }
  catch(e){ return tabs[active].url; }
}
function go(url){ tabs[active].url=url; view.src=url; }
function switchTab(i){ tabs[active].url=current(); active=i; view.src=tabs[i].url; renderTabs(); }
function newTab(){ tabs.push({url:'/shelf',title:'Shelf'}); switchTab(tabs.length-1); }
function closeTab(i){
  if(tabs.length===1) return;
  tabs.splice(i,1);
  if(active>=tabs.length) active=tabs.length-1; else if(i<active) active--;
  view.src=tabs[active].url; renderTabs();
}
view.addEventListener('load', function(){
  var u=current(); tabs[active].url=u; addr.value=u;
  try { tabs[active].title=view.contentDocument.title||u; } catch(e){}
  renderTabs();
});
function submitAddr(){
  var v=addr.value.trim(); if(!v) return;
  if(v.indexOf('http://127.0.0.1')===0){ v=v.slice(16); }
  if(v.charAt(0)==='/'){ go(v); }
  else if(v.indexOf('/')>=0 && v.indexOf(' ')<0){ go('/'+v); }
  else { go('/search?q='+encodeURIComponent(v)); }
}
addr.addEventListener('keydown', function(e){ if(e.key==='Enter') submitAddr(); });
document.getElementById('go').onclick=submitAddr;
document.getElementById('back').onclick=function(){ view.contentWindow.history.back(); };
document.getElementById('fwd').onclick=function(){ view.contentWindow.history.forward(); };
document.getElementById('reload').onclick=function(){ try{ view.contentWindow.location.reload(); }catch(e){ view.src=current(); } };
document.getElementById('shelfbtn').onclick=function(){ go('/shelf'); };
document.getElementById('catbtn').onclick=function(){ go('/catalog'); };
renderTabs();
</script>
</body></html>"##;

fn shelf_page(shelf: &Shelf) -> String {
    let mut items = String::new();
    for b in &shelf.books {
        items.push_str(&format!(
            "<p class=\"item\">- <a href=\"/book/{}\">{}</a> <span class=\"meta\">{} articles, {}</span></p>\n",
            b.id,
            esc(&b.title),
            b.article_count,
            esc(&b.file_name)
        ));
    }
    if shelf.books.is_empty() {
        items = "<p>No books aboard yet. Add .zim files to the shelf directory, or fetch some from the <a href=\"/catalog\">catalog</a>.</p>".to_string();
    }
    format!(
        "<!DOCTYPE html>\n<html><head><meta charset=\"utf-8\"><title>Shelf</title></head>\n\
         <body><h1>The Shelf</h1>\n\
         <form action=\"/search\" method=\"get\"><input name=\"q\" size=\"40\" \
         placeholder=\"Search everything aboard\"> <input type=\"submit\" value=\"Search\"></form>\n\
         <h2>Aboard</h2>\n{}\
         <p><a href=\"/catalog\">Catalog: fetch more books</a></p></body></html>",
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
        body.push_str("");
        for h in &hits {
            let book_title = shelf
                .book(h.book)
                .map(|b| b.title.as_str())
                .unwrap_or_default();
            body.push_str(&format!(
                "<p class=\"item\">- <a href=\"/book/{}/{}\">{}</a> <span class=\"meta\">in {}</span></p>\n",
                h.book,
                h.path,
                esc(if h.title.is_empty() { &h.path } else { &h.title }),
                esc(book_title)
            ));
        }
        body.push_str("");
    }
    format!(
        "<!DOCTYPE html>\n<html><head><meta charset=\"utf-8\"><title>Search</title></head>\n\
         <body><h1>Search: {}</h1>\n{}\n<p><a href=\"/shelf\">Shelf</a></p></body></html>",
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

// ---------------------------------------------------------------------
// The catalog: Kiwix's OPDS feed, read when a wire exists. Offline this
// page simply reports that the catalog cannot be reached; the Shelf
// itself never depends on it.

struct CatalogEntry {
    title: String,
    summary: String,
    publisher: String,
    articles: String,
    ftindex: bool,
    size: u64,
    zim_url: String,
    file_name: String,
}

fn xml_tag(block: &str, tag: &str) -> String {
    let open = format!("<{}>", tag);
    let close = format!("</{}>", tag);
    if let Some(start) = block.find(&open) {
        let rest = &block[start + open.len()..];
        if let Some(end) = rest.find(&close) {
            return rest[..end]
                .replace("&lt;", "<")
                .replace("&gt;", ">")
                .replace("&quot;", "\"")
                .replace("&amp;", "&")
                .trim()
                .to_string();
        }
    }
    String::new()
}

fn xml_attr(tag_text: &str, attr: &str) -> String {
    let pat = format!("{}=\"", attr);
    if let Some(start) = tag_text.find(&pat) {
        let rest = &tag_text[start + pat.len()..];
        if let Some(end) = rest.find('"') {
            return rest[..end].to_string();
        }
    }
    String::new()
}

fn parse_catalog(xml: &str) -> Vec<CatalogEntry> {
    let mut out = Vec::new();
    for block in xml.split("<entry>").skip(1) {
        let block = block.split("</entry>").next().unwrap_or("");
        let acq = match block.find("acquisition/open-access") {
            Some(i) => i,
            None => continue,
        };
        let tag_end = block[acq..].find("/>").map(|e| acq + e).unwrap_or(block.len());
        let tag_text = &block[acq..tag_end];
        let href = xml_attr(tag_text, "href");
        let zim_url = href.strip_suffix(".meta4").unwrap_or(&href).to_string();
        if !zim_url.ends_with(".zim") {
            continue;
        }
        let file_name = zim_url
            .rsplit('/')
            .next()
            .unwrap_or("book.zim")
            .to_string();
        let publisher = match block.find("<publisher>") {
            Some(start) => {
                let rest = &block[start..];
                let end = rest.find("</publisher>").unwrap_or(rest.len());
                xml_tag(&rest[..end], "name")
            }
            None => String::new(),
        };
        out.push(CatalogEntry {
            title: xml_tag(block, "title"),
            summary: xml_tag(block, "summary"),
            publisher,
            articles: xml_tag(block, "articleCount"),
            ftindex: block.contains("_ftindex:yes"),
            size: xml_attr(tag_text, "length").parse().unwrap_or(0),
            zim_url,
            file_name,
        });
    }
    out
}

fn pretty_category(cat: &str) -> String {
    let spaced = cat.replace('_', " ");
    let mut chars = spaced.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => spaced,
    }
}

fn parse_categories(xml: &str) -> Vec<String> {
    let mut out = Vec::new();
    for block in xml.split("<entry>").skip(1) {
        let block = block.split("</entry>").next().unwrap_or("");
        let title = xml_tag(block, "title");
        if !title.is_empty() {
            out.push(title);
        }
    }
    out
}

/// The category shelf, cached for ten minutes: it changes rarely, and
/// the catalog page should not pay a second fetch for it every visit.
fn categories(state: &AppState) -> Vec<String> {
    const TTL_SECS: u64 = 600;
    {
        let cache = state.cats.read().unwrap();
        if let Some((when, cats)) = &*cache {
            if when.elapsed().as_secs() < TTL_SECS {
                return cats.clone();
            }
        }
    }
    let fetched = state
        .agent
        .get("https://library.kiwix.org/catalog/v2/categories")
        .call()
        .and_then(|r| r.into_string().map_err(ureq::Error::from));
    match fetched {
        Ok(xml) => {
            let cats = parse_categories(&xml);
            *state.cats.write().unwrap() = Some((std::time::Instant::now(), cats.clone()));
            cats
        }
        Err(_) => state
            .cats
            .read()
            .unwrap()
            .as_ref()
            .map(|(_, cats)| cats.clone())
            .unwrap_or_default(),
    }
}

fn catalog_page(state: &AppState, query: &str, lang: &str, cat: &str) -> String {
    let lang = if lang.is_empty() { "eng" } else { lang };
    let form = format!(
        "<form action=\"/catalog\" method=\"get\">\
         <input name=\"q\" size=\"32\" value=\"{}\" placeholder=\"Search the catalog\"> \
         <input name=\"lang\" size=\"4\" value=\"{}\"> \
         <input type=\"hidden\" name=\"c\" value=\"{}\"> \
         <input type=\"submit\" value=\"Search\"></form>",
        esc(query),
        esc(lang),
        esc(cat)
    );

    let mut catbar = String::from("<p class=\"cats\">");
    if cat.is_empty() {
        catbar.push_str("<strong>All</strong>");
    } else {
        catbar.push_str(&format!(
            "<a href=\"/catalog?lang={}\">All</a>",
            url_encode(lang)
        ));
    }
    for c in categories(state) {
        catbar.push_str(" | ");
        if c == cat {
            catbar.push_str(&format!("<strong>{}</strong>", esc(&pretty_category(&c))));
        } else {
            catbar.push_str(&format!(
                "<a href=\"/catalog?c={}&lang={}\">{}</a>",
                url_encode(&c),
                url_encode(lang),
                esc(&pretty_category(&c))
            ));
        }
    }
    catbar.push_str("</p>");

    let mut url = "https://library.kiwix.org/catalog/v2/entries?count=24".to_string();
    url.push_str(&format!("&lang={}", url_encode(lang)));
    if !query.is_empty() {
        url.push_str(&format!("&q={}", url_encode(query)));
    }
    if !cat.is_empty() {
        url.push_str(&format!("&category={}", url_encode(cat)));
    }

    let fetched = state
        .agent
        .get(&url)
        .call()
        .and_then(|r| r.into_string().map_err(ureq::Error::from));
    let body = match fetched {
        Ok(xml) => {
            let entries = parse_catalog(&xml);
            if entries.is_empty() {
                "<p>The catalog has nothing by that name.</p>".to_string()
            } else {
                let shelf = state.shelf();
                let mut groups: std::collections::BTreeMap<String, Vec<&CatalogEntry>> =
                    std::collections::BTreeMap::new();
                for e in &entries {
                    let key = if e.publisher.is_empty() {
                        "Other sources".to_string()
                    } else {
                        e.publisher.clone()
                    };
                    groups.entry(key).or_default().push(e);
                }
                let mut out = String::new();
                for (publisher, es) in &groups {
                    out.push_str(&format!("<h2>{}</h2>\n", esc(publisher)));
                    for e in es {
                        let aboard = shelf.books.iter().any(|b| b.file_name == e.file_name);
                        let meta = format!(
                            "{} articles, {:.1} MB, full text index: {}",
                            esc(&e.articles),
                            e.size as f64 / 1_000_000.0,
                            if e.ftindex { "yes" } else { "no" }
                        );
                        let action = if aboard {
                            "<span class=\"meta\">aboard</span>".to_string()
                        } else {
                            format!(
                                "<a href=\"/acquire?url={}&name={}&size={}\">Add to the Shelf</a>",
                                url_encode(&e.zim_url),
                                url_encode(&e.file_name),
                                e.size
                            )
                        };
                        out.push_str(&format!(
                            "<p class=\"item\">- <strong>{}</strong> - {}<br><span class=\"meta\">{}</span> {}</p>\n",
                            esc(&e.title),
                            esc(&e.summary),
                            meta,
                            action
                        ));
                    }
                    out.push_str("");
                }
                out
            }
        }
        Err(e) => format!(
            "<p>The catalog could not be reached ({}). The Shelf works without it; \
             this page wakes up when a connection does.</p>",
            esc(&e.to_string())
        ),
    };
    format!(
        "<!DOCTYPE html>\n<html><head><meta charset=\"utf-8\"><title>Catalog</title></head>\n\
         <body><h1>Catalog</h1>\n{}\n{}\n{}\n<p><a href=\"/shelf\">Shelf</a></p></body></html>",
        form, catbar, body
    )
}

// ---------------------------------------------------------------------
// Fetching a book: the download runs on its own thread and reports its
// state here, so the progress page (and anything else) can watch it
// come down. One fetch at a time; the Shelf keeps serving throughout.

#[derive(Clone, Copy, PartialEq)]
enum Phase {
    Idle,
    Running,
    Done,
    Failed,
}

struct Download {
    name: String,
    received: u64,
    total: u64,
    phase: Phase,
    book: usize,
    title: String,
    error: String,
}

impl Download {
    fn idle() -> Self {
        Download {
            name: String::new(),
            received: 0,
            total: 0,
            phase: Phase::Idle,
            book: 0,
            title: String::new(),
            error: String::new(),
        }
    }
}

fn download_json(state: &AppState) -> String {
    let d = state.download.read().unwrap();
    let phase = match d.phase {
        Phase::Idle => "idle",
        Phase::Running => "running",
        Phase::Done => "done",
        Phase::Failed => "failed",
    };
    let percent = if d.total > 0 {
        d.received * 100 / d.total
    } else {
        0
    };
    format!(
        "{{\"phase\":\"{}\",\"name\":\"{}\",\"received\":{},\"total\":{},\"percent\":{},\"book\":{},\"title\":\"{}\",\"error\":\"{}\"}}",
        phase,
        json_esc(&d.name),
        d.received,
        d.total,
        percent,
        d.book,
        json_esc(&d.title),
        json_esc(&d.error)
    )
}

const PROGRESS_PAGE: &str = r##"<!DOCTYPE html>
<html><head><meta charset="utf-8"><title>Fetching</title>
<style>
body{font-family:system-ui,sans-serif;margin:24px;color:#111}
#track{height:22px;background:#eee;border:1px solid #bbb;max-width:640px}
#bar{height:100%;width:0;background:#3b6fd4;transition:width .4s}
</style></head>
<body>
<h1 id="head">Fetching</h1>
<div id="track"><div id="bar"></div></div>
<p id="line">starting</p>
<p id="done"></p>
<p><a href="/shelf">Shelf</a> - reading keeps working while a book comes down.</p>
<script>
function mb(n){ return (n/1000000).toFixed(1) + ' MB'; }
function tick(){
  fetch('/api/acquire-status').then(function(r){ return r.json(); }).then(function(d){
    if(d.name){ document.getElementById('head').textContent = 'Fetching ' + d.name; }
    var line = document.getElementById('line');
    if(d.phase === 'running'){
      if(d.total > 0){
        document.getElementById('bar').style.width = d.percent + '%';
        line.textContent = mb(d.received) + ' of ' + mb(d.total) + ' - ' + d.percent + '%';
      } else {
        line.textContent = mb(d.received) + ' so far';
      }
      setTimeout(tick, 600);
    } else if(d.phase === 'done'){
      document.getElementById('bar').style.width = '100%';
      line.textContent = 'down: ' + mb(d.received) + '.';
      document.getElementById('done').innerHTML =
        '<a href="/book/' + d.book + '">' + d.title + '</a> is aboard. <a href="/shelf">Open the Shelf</a>';
    } else if(d.phase === 'failed'){
      line.textContent = 'the fetch did not finish: ' + d.error;
    } else {
      line.textContent = 'nothing is fetching right now.';
    }
  }).catch(function(){ setTimeout(tick, 1500); });
}
tick();
</script>
</body></html>"##;

fn acquire(state: Arc<AppState>, url: &str, name: &str, size: u64) -> (u16, String) {
    let name_ok = !name.is_empty()
        && name.ends_with(".zim")
        && !name.contains("..")
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'));
    if !name_ok {
        return (400, "<p>That file name is not one the Shelf can take.</p>".to_string());
    }
    if !url.starts_with("https://") || !url.ends_with(".zim") {
        return (400, "<p>That is not a book address.</p>".to_string());
    }

    {
        let mut d = state.download.write().unwrap();
        if d.phase == Phase::Running {
            return (
                409,
                format!(
                    "<p>Already fetching {} - <a href=\"/acquire-progress\">watch it come down</a>.</p>",
                    esc(&d.name)
                ),
            );
        }
        *d = Download {
            name: name.to_string(),
            received: 0,
            total: size,
            phase: Phase::Running,
            book: 0,
            title: String::new(),
            error: String::new(),
        };
    }

    let url = url.to_string();
    let name = name.to_string();
    std::thread::spawn(move || run_download(state, url, name));
    (200, PROGRESS_PAGE.to_string())
}

fn download_failed(state: &AppState, part: &std::path::Path, msg: String) {
    let _ = std::fs::remove_file(part);
    let mut d = state.download.write().unwrap();
    d.phase = Phase::Failed;
    d.error = msg;
}

fn run_download(state: Arc<AppState>, url: String, name: String) {
    use std::io::Write;

    let part = state.dir.join(format!("{}.part", name));
    let dest = state.dir.join(&name);

    let response = match state.agent.get(&url).call() {
        Ok(r) => r,
        Err(e) => {
            download_failed(&state, &part, format!("fetch failed: {}", e));
            return;
        }
    };
    if let Some(len) = response
        .header("Content-Length")
        .and_then(|v| v.parse::<u64>().ok())
    {
        if len > 0 {
            state.download.write().unwrap().total = len;
        }
    }

    let mut file = match std::fs::File::create(&part) {
        Ok(f) => f,
        Err(e) => {
            download_failed(&state, &part, e.to_string());
            return;
        }
    };
    let mut reader = response.into_reader();
    let mut buf = [0u8; 65536];
    let mut received: u64 = 0;
    loop {
        match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                if let Err(e) = file.write_all(&buf[..n]) {
                    download_failed(&state, &part, e.to_string());
                    return;
                }
                received += n as u64;
                state.download.write().unwrap().received = received;
            }
            Err(e) => {
                download_failed(&state, &part, e.to_string());
                return;
            }
        }
    }
    drop(file);

    let magic_ok = std::fs::File::open(&part)
        .and_then(|mut f| {
            let mut head = [0u8; 3];
            f.read_exact(&mut head).map(|_| head)
        })
        .map(|head| &head == b"ZIM")
        .unwrap_or(false);
    if !magic_ok {
        download_failed(&state, &part, "the download is not a ZIM book".to_string());
        return;
    }
    if let Err(e) = std::fs::rename(&part, &dest) {
        download_failed(&state, &part, e.to_string());
        return;
    }

    state.reload();
    let shelf = state.shelf();
    let mut d = state.download.write().unwrap();
    match shelf.books.iter().find(|b| b.file_name == name) {
        Some(book) => {
            d.phase = Phase::Done;
            d.book = book.id;
            d.title = book.title.clone();
        }
        None => {
            d.phase = Phase::Failed;
            d.error = "downloaded, but the book would not open".to_string();
        }
    }
}

// ---------------------------------------------------------------------

struct AppState {
    dir: PathBuf,
    shelf: RwLock<Arc<Shelf>>,
    download: RwLock<Download>,
    cats: RwLock<Option<(std::time::Instant, Vec<String>)>>,
    agent: ureq::Agent,
}

impl AppState {
    fn shelf(&self) -> Arc<Shelf> {
        self.shelf.read().unwrap().clone()
    }

    fn reload(&self) {
        let fresh = Arc::new(Shelf::open_dir(&self.dir));
        *self.shelf.write().unwrap() = fresh;
    }
}

fn handle(request: tiny_http::Request, state: Arc<AppState>) {
    let url = request.url().to_string();
    let path = url.split('?').next().unwrap_or("/").to_string();
    let shelf = state.shelf();

    let response = if path == "/" {
        html_response(CHROME.to_string())
    } else if path == "/shelf" {
        html_response(shelf_page(&shelf))
    } else if path == "/favicon.ico" {
        Response::from_data(Vec::new()).with_status_code(204)
    } else if path == "/api/books" {
        json_response(books_json(&shelf))
    } else if path == "/search" {
        let q = query_param(&url, "q").unwrap_or_default();
        html_response(search_page(&shelf, &q))
    } else if path == "/api/search" {
        let q = query_param(&url, "q").unwrap_or_default();
        json_response(search_json(&shelf, &q))
    } else if path == "/catalog" {
        let q = query_param(&url, "q").unwrap_or_default();
        let lang = query_param(&url, "lang").unwrap_or_else(|| "eng".to_string());
        let c = query_param(&url, "c").unwrap_or_default();
        html_response(catalog_page(&state, &q, &lang, &c))
    } else if path == "/acquire" {
        let u = query_param(&url, "url").unwrap_or_default();
        let n = query_param(&url, "name").unwrap_or_default();
        let size: u64 = query_param(&url, "size")
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        let (status, body) = acquire(state.clone(), &u, &n, size);
        html_response(body).with_status_code(status)
    } else if path == "/acquire-progress" {
        html_response(PROGRESS_PAGE.to_string())
    } else if path == "/api/acquire-status" {
        json_response(download_json(&state))
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
                            Header::from_bytes(&b"Location"[..], location.as_bytes()).unwrap(),
                        )
                } else {
                    let decoded = url_decode(&item_path);
                    match book.get(&decoded) {
                        Ok(Some((mime, bytes))) => Response::from_data(bytes).with_header(
                            Header::from_bytes(&b"Content-Type"[..], mime.as_bytes()).unwrap(),
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

    // The catalog fetch honors a proxy when the environment names one;
    // on a plain network there is none and the connection is direct.
    let mut agent_builder = ureq::AgentBuilder::new()
        .user_agent("Umiak/0.1 (+https://freecomputation.org)")
        .timeout_connect(std::time::Duration::from_secs(15));
    for var in ["HTTPS_PROXY", "https_proxy", "HTTP_PROXY", "http_proxy", "ALL_PROXY", "all_proxy"] {
        if let Ok(p) = std::env::var(var) {
            if let Ok(proxy) = ureq::Proxy::new(&p) {
                agent_builder = agent_builder.proxy(proxy);
                break;
            }
        }
    }

    let dir = PathBuf::from(&shelf_dir);
    let state = Arc::new(AppState {
        shelf: RwLock::new(Arc::new(Shelf::open_dir(&dir))),
        download: RwLock::new(Download::idle()),
        cats: RwLock::new(None),
        dir,
        agent: agent_builder.build(),
    });
    println!(
        "umiak-server: {} book(s) aboard from {}",
        state.shelf().books.len(),
        shelf_dir
    );

    let server = Server::http(("127.0.0.1", port)).expect("bind loopback");
    println!("umiak-server: listening on http://127.0.0.1:{}/", port);

    for request in server.incoming_requests() {
        let state = state.clone();
        std::thread::spawn(move || handle(request, state));
    }
}
