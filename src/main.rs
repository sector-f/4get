extern crate hyper;
extern crate scraper;
extern crate num_cpus;
extern crate regex;
extern crate threadpool;

use hyper::client::Client;
use regex::Regex;
use scraper::{Html, Selector};
use std::env::args;
use std::path::Path;
use std::sync::mpsc::channel;
use std::fs::OpenOptions;
use std::io::{stderr, Read, Write};
use threadpool::ThreadPool;

fn validate_arg(regex: &regex::Regex, arg: &str) -> bool {
    match regex.is_match(arg) {
        true => true,
        false => {
            let _ = writeln!(stderr(), "Invalid URL: {}", arg);
            false
        },
    }
}

fn get_page(url: &str) -> Result<String, hyper::Error> {
    let client = Client::new();
    let mut page = String::new();
    let mut response = try!(client.get(url).send());
    let _ = response.read_to_string(&mut page);
    return Ok(page)
}

fn parse_html(html: &str) -> Vec<String> {
    let document = Html::parse_document(&html);
    let thumbnail = Selector::parse("a.fileThumb").unwrap();

    let mut vector = Vec::new();

    for item in document.select(&thumbnail) {
        if let Some(fragment) = item.value().attr("href") {
            let url = format!("https:{}", fragment);
            vector.push(url);
        }
    }

    vector

    // document.select(&thumbnail).into_iter()
    //     .map(|post| format!("https:{}", post.value().attr("href").unwrap_or("")))
    //     .collect::<Vec<_>>()
}

fn download_file(url: &Path) {
    if let Some(file_name) = url.file_name() {
        let saved_file = OpenOptions::new()
            .create(true)
            .write(true)
            .open(file_name);

        if let Ok(mut file) = saved_file {
            let client = Client::new();
            let mut page = Vec::new();
            let _ = writeln!(stderr(), "Downloading {}...", &url.display());
            if let Ok(mut response) = client.get(url.to_str().unwrap()).send() {
                let _ = response.read_to_end(&mut page);
                let _ = file.write_all(&page);
            }
        }
    }
}

fn main() {
    let url_re = Regex::new(r"https{0,1}://boards.4chan.org/\S+/thread/\d+").unwrap();

    let arguments: Vec<String> =
        args().skip(1).into_iter()
        .filter(|arg| validate_arg(&url_re, &arg)).collect();

    let urls =
        arguments.into_iter()
        .flat_map(|url| Result::ok(get_page(&url)))
        .flat_map(|s| parse_html(&s))
        .collect::<Vec<_>>();

    let cpus = num_cpus::get();
    let pool = ThreadPool::new(cpus);
    let (tx, rx) = channel::<Result<(), ()>>();

    for url in &urls {
        let tx = tx.clone();
        let url = url.clone();
        pool.execute(move|| {
            let _ = tx;
            download_file(Path::new(&url));
        });
    }

    drop(tx);
    let mut counter: usize = 0;
    while counter < urls.len() {
        let _ = rx.recv();
        counter += 1;
    }
}
