extern crate hyper;
extern crate scraper;
extern crate num_cpus;
extern crate regex;
extern crate threadpool;
extern crate clap;

use clap::{App, Arg};
use hyper::client::Client;
use regex::Regex;
use scraper::{Html, Selector};
use std::path::Path;
use std::sync::mpsc::channel;
use std::ffi::OsStr;
use std::fs::OpenOptions;
use std::io::{stderr, Read, Write};
use threadpool::ThreadPool;

fn validate_arg(regex: &regex::Regex, arg: &OsStr) -> bool {
    let arg = arg.to_string_lossy().into_owned();
    match regex.is_match(&arg) {
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

    document.select(&thumbnail).into_iter()
        .filter_map(|item| item.value().attr("href"))
        .map(|url| format!("https:{}", url))
        .collect()
}

fn download_file(url: &Path) {
    if let Some(file_name) = url.file_name() {

        if Path::new(&file_name).exists() {
            return;
        }

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
    let matches = App::new("4get")
        .version(option_env!("CARGO_PKG_VERSION").unwrap_or("unknown version"))
        .about("Downloads images from 4chan threads")
        .arg(Arg::with_name("URL")
            .help("4chan thread URL to download images from")
            .index(1)
            .multiple(true)
            .required(true))
        .get_matches();

    let url_re = Regex::new(r"https?://boards.4chan.org/\S+/thread/\d+/\S+").unwrap();

    let arguments: Vec<&OsStr> =
        matches.values_of_os("URL").unwrap().into_iter()
        .filter(|arg| validate_arg(&url_re, &arg)).collect();

    let urls =
        arguments.into_iter()
        .flat_map(|url| Result::ok(get_page(&url.to_string_lossy())))
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
