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
use std::path::PathBuf;
use std::sync::mpsc::channel;
use std::ffi::OsStr;
use std::fs::OpenOptions;
use std::io::{stderr, Read, Write};
use threadpool::ThreadPool;

#[derive(Clone)]
struct DownloadTarget {
    url: String,
    filename: PathBuf,
}

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

fn parse_html(html: &str, use_original: bool) -> Vec<DownloadTarget> {
    let document = Html::parse_document(&html);
    let thumbnail = Selector::parse("div.fileText > a").unwrap();

    let mut files_vec = Vec::new();

    for image in document.select(&thumbnail) {
        if let Some(fragment) = image.value().attr("href") {
            files_vec.push(
                DownloadTarget {
                    url: format!("https:{}", fragment),
                    filename:
                        match use_original {
                            false => {
                                if let Some(name) = PathBuf::from(&fragment).file_name() {
                                    PathBuf::from(name)
                                } else {
                                    continue
                                }
                            },
                            true => {
                                if let Some(title) = image.value().attr("title") {
                                    PathBuf::from(title)
                                } else {
                                    PathBuf::from(image.inner_html())
                                }
                            },
                        }
                }
            );
        }
    }

    files_vec
}

fn download_file(target: DownloadTarget) {
    if target.filename.exists() {
        return;
    }

    let saved_file = OpenOptions::new()
        .create(true)
        .write(true)
        .open(&target.filename);

    if let Ok(mut file) = saved_file {
        let client = Client::new();
        let mut page = Vec::new();
        let _ = writeln!(stderr(), "Downloading {}...", &target.filename.display());
        if let Ok(mut response) = client.get(&target.url).send() {
            let _ = response.read_to_end(&mut page);
            let _ = file.write_all(&page);
        }
    }
}

fn is_positive_int(n: String) -> Result<(), String> {
    match n.parse::<usize>() {
        Ok(val) => {
            if val == 0 {
                Err(String::from("CONCURRENT UPLOADS cannot be zero"))
            } else {
                Ok(())
            }
        },
        Err(_) => Err(String::from("CONCURRENT UPLOADS must be a positive integer")),
    }
}

fn main() {
    let matches = App::new("4get")
        .version(option_env!("CARGO_PKG_VERSION").unwrap_or("unknown version"))
        .about("Downloads images from 4chan threads")
        .arg(Arg::with_name("downloads")
             .help("Number of simultaneous downloads")
             .short("d")
             .long("downloads")
             .validator(is_positive_int)
             .takes_value(true))
        .arg(Arg::with_name("original-name")
             .help("Save file with original name")
             .short("o")
             .long("original"))
        .arg(Arg::with_name("URL")
            .help("4chan thread URL to download images from")
            .index(1)
            .multiple(true)
            .required(true))
        .get_matches();

    let concurrent: usize = matches.value_of("downloads")
        .and_then(|s| s.parse().ok())
        .unwrap_or(num_cpus::get());

    let use_original = matches.is_present("original-name");

    let url_re = Regex::new(r"https?://boards.4chan.org/\S+/thread/\d+/\S+").unwrap();

    let arguments: Vec<&OsStr> =
        matches.values_of_os("URL").unwrap().into_iter()
        .filter(|arg| validate_arg(&url_re, &arg)).collect();

    let files =
        arguments.into_iter()
        .flat_map(|url| Result::ok(get_page(&url.to_string_lossy())))
        .flat_map(|s| parse_html(&s, use_original))
        .collect::<Vec<_>>();

    let pool = ThreadPool::new(concurrent);
    let (tx, rx) = channel::<Result<(), ()>>();

    for file in &files {
        let tx = tx.clone();
        let file = file.clone();
        pool.execute(move|| {
            let _ = tx;
            download_file(file);
        });
    }

    drop(tx);
    let mut counter: usize = 0;
    while counter < files.len() {
        let _ = rx.recv();
        counter += 1;
    }
}
