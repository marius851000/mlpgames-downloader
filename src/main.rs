use std::{collections::{BTreeMap, HashMap}, fs::File, path::PathBuf, sync::Arc};
use actix_files::NamedFile;
use serde::Deserialize;
use actix_web::{App, Either, HttpResponseBuilder, HttpServer, Responder, get, http::{StatusCode, header::ContentDisposition}, web::{self, Data}};
use anyhow::Context;
use clap::Parser;
use maud::{html, DOCTYPE};

#[derive(Deserialize)]
struct MetaJson {
    modified: f64,
    path: String,
    size: u64
}

#[derive(Clone, Debug)]
struct Entry {
    file_name: String
}

#[derive(Clone, Debug)]
struct InfoHolder {
    pub map: BTreeMap<String, Entry>,
    pub folder: PathBuf
}

impl InfoHolder {
    pub fn parse(folder: &std::path::Path) -> anyhow::Result<Self> {
        let mut map = BTreeMap::new();
        //TODO: par_iter?
        for path in std::fs::read_dir(folder).context("trying to listing files")? {
            let path_info = path.context("trying to list a spectific file")?;
            if path_info.file_name().to_string_lossy().ends_with(".meta.json") {
                let mut json_file = File::open(&path_info.path()).with_context(|| format!("Trying to open {:?}", path_info.path()))?;
                let meta: MetaJson = serde_json::from_reader(&mut json_file).with_context(|| format!("Trying to read {:?}", path_info.path()))?;
                let file_id = path_info.file_name().to_string_lossy().split(".").next().expect("split should at least return one result, right?").to_string();
                map.insert(file_id, Entry {
                    file_name: meta.path
                });
            }
        };

        Ok(Self {
            map,
            folder: folder.to_path_buf()
        })
    }
}

#[derive(Parser, Debug)]
#[command(version, about)]
struct Settings {
    source_path: PathBuf,
    host: String,
    port: u16
}

//TODO: cache
fn render_listing(holder: &InfoHolder) -> maud::Markup {
    html! {
        (DOCTYPE)
        html {
            head {
                meta charset="utf-8"
                title { "MLP games archive file list" }
            }
            body {
                h1 { "MLP games archive file list" }
                ul {
                    @for entry in holder.map.iter() {
                        li {
                            a href=(format!("./{}", entry.0)) { (entry.0) } " (" (entry.1.file_name) ")"
                        }
                    }
                }
            }
        }
    }
}

#[get("/")]
async fn index(info: Data<Arc<InfoHolder>>) -> impl Responder {
    render_listing(&info)
}

#[get("/{id}")]
async fn hello(path: web::Path<String>, info: Data<Arc<InfoHolder>>) -> impl Responder {
    let entry = if let Some(entry) = info.map.get(path.as_ref()) {
        entry
    } else {
        return Either::Right(HttpResponseBuilder::new(StatusCode::NOT_FOUND).body("file not found"))
    };
    // no risk for path traversel or anything like that, we already checked the file is valid and exist the previous step
    let named_file = match NamedFile::open(info.folder.join(path.as_ref())) {
        Ok(v) => v,
        Err(e) => {
            log::error!("Error at the path {:?}: {:?}", path.as_str(), e);
            return Either::Right(HttpResponseBuilder::new(StatusCode::INTERNAL_SERVER_ERROR).body("The file that was expected to be here can’t be opened by the server!?!?"));
        }
    };

    let named_file = named_file.set_content_disposition(ContentDisposition::attachment(entry.file_name.to_string()));

    return Either::Left(named_file)
}


#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();
    let settings = Settings::parse();
    let info: Data<Arc<InfoHolder>> = Data::new(Arc::new(InfoHolder::parse(&settings.source_path).unwrap()));

    HttpServer::new(move || App::new().app_data(info.clone()).service(index).service(hello))
        .bind((settings.host, settings.port))?
        .run()
        .await
}
