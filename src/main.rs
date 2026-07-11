use actix_files::NamedFile;
use actix_web::{
    App, Either, HttpResponseBuilder, HttpServer, Responder,
    http::{StatusCode, header::ContentDisposition},
    route,
    web::{self, Data},
};
use anyhow::Context;
use atomic_swapping::AtomicSwap;
use clap::Parser;
use log::{error, info};
use maud::{DOCTYPE, html};
use notify::{Config, EventHandler, PollWatcher, Watcher};
use serde::Deserialize;
use std::{collections::BTreeMap, fs::File, path::PathBuf, sync::Arc, time::Duration};
use tokio::sync::watch::Sender;

#[derive(Deserialize)]
struct MetaJson {
    #[allow(unused)]
    modified: f64,
    path: String,
    #[allow(unused)]
    size: u64,
}

#[derive(Clone, Debug)]
struct Entry {
    file_name: String,
}

#[derive(Clone, Debug)]
struct InfoHolder {
    pub map: BTreeMap<String, Entry>,
    pub folder: PathBuf,
}

impl InfoHolder {
    pub fn parse(folder: &std::path::Path) -> anyhow::Result<Self> {
        let mut map = BTreeMap::new();
        //TODO: par_iter?
        for path in std::fs::read_dir(folder).context("trying to listing files")? {
            let path_info = path.context("trying to list a spectific file")?;
            if path_info
                .file_name()
                .to_string_lossy()
                .ends_with(".meta.json")
            {
                let mut json_file = File::open(&path_info.path())
                    .with_context(|| format!("Trying to open {:?}", path_info.path()))?;
                let meta: MetaJson = serde_json::from_reader(&mut json_file)
                    .with_context(|| format!("Trying to read {:?}", path_info.path()))?;
                let file_id = path_info
                    .file_name()
                    .to_string_lossy()
                    .split(".")
                    .next()
                    .expect("split should at least return one result, right?")
                    .to_string();
                map.insert(
                    file_id,
                    Entry {
                        file_name: meta.path,
                    },
                );
            }
        }

        Ok(Self {
            map,
            folder: folder.to_path_buf(),
        })
    }
}

#[derive(Parser, Debug)]
#[command(version, about)]
struct Settings {
    source_path: PathBuf,
    host: String,
    port: u16,
}

//TODO: cache
fn render_listing(holder: &InfoHolder) -> maud::Markup {
    html! {
        (DOCTYPE)
        html {
            head {
                meta charset="utf-8";
                title { "MLP games archive file list" };
            }
            body {
                h1 { "MLP games archive file list" };
                ul {
                    @for entry in holder.map.iter() {
                        li {
                            a href=(format!("./{}", entry.0)) { (entry.0) } " (" (entry.1.file_name) ")";
                        }
                    }
                }
            }
        }
    }
}

#[route("/", method = "GET", method = "HEAD")]
async fn index(info: Data<Arc<AtomicSwap<Arc<InfoHolder>>>>) -> impl Responder {
    render_listing(&info.clone_inner())
}

#[route("/{id}", method = "GET", method = "HEAD")]
async fn hello(
    path: web::Path<String>,
    info: Data<Arc<AtomicSwap<Arc<InfoHolder>>>>,
) -> impl Responder {
    let info = info.clone_inner();
    let entry = if let Some(entry) = info.map.get(path.as_ref()) {
        entry
    } else {
        return Either::Right(
            HttpResponseBuilder::new(StatusCode::NOT_FOUND).body("file not found"),
        );
    };
    // no risk for path traversel or anything like that, we already checked the file is valid and exist the previous step
    let named_file = match NamedFile::open(info.folder.join(path.as_ref())) {
        Ok(v) => v,
        Err(e) => {
            log::error!("Error at the path {:?}: {:?}", path.as_str(), e);
            return Either::Right(
                HttpResponseBuilder::new(StatusCode::INTERNAL_SERVER_ERROR).body(
                    "The file that was expected to be here can’t be opened by the server!?!?",
                ),
            );
        }
    };

    let named_file = named_file
        .set_content_disposition(ContentDisposition::attachment(entry.file_name.to_string()));

    return Either::Left(named_file);
}

#[derive(Default)]
struct FileChanged {
    tx: Sender<()>,
}

impl EventHandler for FileChanged {
    fn handle_event(&mut self, _event: notify::Result<notify::Event>) {
        self.tx.send(()).unwrap();
        info!("folder change noticed");
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let (tx, mut rx) = tokio::sync::watch::channel(());
    env_logger::init();
    let settings = Settings::parse();
    let info = Arc::new(AtomicSwap::new(Arc::new(
        InfoHolder::parse(&settings.source_path).unwrap(),
    )));

    // TODO: actually use 60*2
    let mut watcher = PollWatcher::new(
        FileChanged { tx },
        Config::default().with_poll_interval(Duration::from_secs(60)),
    )
    .unwrap();
    watcher
        .watch(&settings.source_path, notify::RecursiveMode::Recursive)
        .unwrap();
    watcher.poll().unwrap();

    let info_for_update = info.clone();
    tokio::spawn(async move {
        loop {
            rx.changed().await.unwrap();
            match InfoHolder::parse(&settings.source_path) {
                Ok(v) => {
                    info_for_update.swap(Arc::new(v));
                    info!("settings reloaded");
                }
                Err(e) => {
                    error!("Error updating data: {:?}", e);
                }
            };
        }
    });

    HttpServer::new(move || {
        App::new()
            .app_data(Data::new(info.clone()))
            .service(index)
            .service(hello)
    })
    .bind((settings.host, settings.port))?
    .run()
    .await
}
