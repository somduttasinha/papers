use axum::body::Bytes;
use axum::response::IntoResponse;
use serde_json::json;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::Path;
use std::sync::Arc;
use tantivy::TantivyDocument;
use tokio::process::Command;
use tokio::sync::Mutex;

use axum::extract::{Query, State, multipart};
use axum::routing::{get, post};
use axum::{Json, Router};
use tantivy::collector::{Count, TopDocs};
use tantivy::query::QueryParser;
use tantivy::schema::{STORED, Schema, TEXT, Value};
use tantivy::{Document, Index, IndexWriter, ReloadPolicy, doc};
use tempfile::TempDir;
use tokio::fs;
use tower_http::cors::Any;
use tower_http::cors::CorsLayer;

mod utils;

struct AppState {
    index: Index,
    schema: Schema,
    writer: Mutex<IndexWriter>,
}

#[tokio::main]
async fn main() -> tantivy::Result<()> {
    let index_path = TempDir::new()?;

    let mut schema_builder = Schema::builder();

    let title = schema_builder.add_text_field("title", TEXT | STORED);

    let body = schema_builder.add_text_field("body", TEXT);

    let schema = schema_builder.build();

    let index = Index::create_in_dir(&index_path, schema.clone())?;

    let mut index_writer: IndexWriter = index.writer(50_000_000)?;

    let directory_path = Path::new("tmp/docs");
    match fs::read_dir(directory_path).await {
        Ok(mut entries) => {
            while let Ok(entry) = entries.next_entry().await {
                match entry {
                    None => break,
                    Some(entry) => match entry.path().extension().and_then(OsStr::to_str) {
                        Some("pdf") => {
                            let path = entry.path();

                            let contents = utils::pdf_to_string(&path).await;
                            println!("Output: {}", contents);

                            let file_name = path.file_name().unwrap().to_str().unwrap();

                            println!("Processing: {}", &file_name);
                            index_writer.add_document(doc!(
                                title => file_name,
                                body => contents
                            ))?;
                        }
                        Some(_) | None => println!("{}", entry.path().display()),
                    },
                }
            }
        }
        Err(_) => todo!(),
    }

    index_writer.commit()?;

    let state = Arc::new(AppState {
        index,
        schema,
        writer: Mutex::<tantivy::IndexWriter>::new(index_writer),
    });

    let upload_routes: Router<()> = Router::new()
        .route("/doc", post(save_and_upsert))
        .with_state(Arc::clone(&state));

    let search_routes: Router<()> = Router::new()
        .route("/", get(find_matches))
        .with_state(Arc::clone(&state));

    let api_routes = Router::new()
        .nest("/upload", upload_routes)
        .nest("/search", search_routes);

    let app = Router::new().nest("/api", api_routes).layer(
        CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any),
    );

    let listener = tokio::net::TcpListener::bind("10.20.10.1:8080")
        .await
        .unwrap();

    axum::serve(listener, app).await.unwrap();

    Ok(())
}

/// We get the multipart form data as a stream of fields, to avoid overloading RAM for large files,
/// we will save to disk as we receive them and build the tantivy::document in memory, we only
/// commit once we have all the data for atomicity.
async fn save_and_upsert(State(state): State<Arc<AppState>>, mut multipart: multipart::Multipart) {
    let mut doc = TantivyDocument::new();
    let schema = &state.schema;

    let title_field = schema.get_field("title").expect("Expected a title field");
    let body_field = schema.get_field("body").expect("Expected a body field");

    let mut set_field = false;

    while let Some(field) = multipart.next_field().await.unwrap() {
        let name = field.name().unwrap().to_string();
        let filename = field.file_name().unwrap().to_string();

        if !set_field {
            doc.add_text(title_field, &filename);
            set_field = true;
        }

        let data: Bytes = field.bytes().await.unwrap();

        println!("Length of `{}` is {} bytes", name, data.len());
        if name == "file" {
            tokio::fs::write(format!("tmp/docs/{}", filename), &data)
                .await
                .unwrap();

            let contents =
                utils::pdf_to_string(&Path::new(&format!("tmp/docs/{}", filename))).await;

            println!(
                "Managed to get the contents: {}",
                contents
                    .split_whitespace()
                    .take(10)
                    .collect::<Vec<&str>>()
                    .join(" ")
            );

            doc.add_text(body_field, contents);
        }
    }

    let mut index_writer = state.writer.lock().await; // by using await we know that we aren't
    // blocking the runtime
    index_writer.add_document(doc).unwrap();
    index_writer.commit().unwrap();
}

async fn find_matches(
    State(state): State<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>,
) -> Json<Vec<String>> {
    let index = &state.index;
    let schema = &state.schema;

    let reader = index
        .reader_builder()
        .reload_policy(ReloadPolicy::OnCommitWithDelay)
        .try_into()
        .unwrap();

    let searcher = reader.searcher();

    let title = schema.get_field("title").expect("Expected a title field");
    let body = schema.get_field("body").expect("Expected a body field");

    println!("params: {:#?}", params);
    let query_term = params.get("query").unwrap();

    println!("Query: {}", query_term);

    let query_parser = QueryParser::for_index(&index, vec![title, body]);
    let query = query_parser
        .parse_query(&query_term)
        .expect("Expected a valid query");

    let (top_docs, count) = searcher
        .search(&query, &(TopDocs::with_limit(5), Count))
        .unwrap();

    //for (score, doc_address) in top_docs {
    //    let retrieved_doc: TantivyDocument = searcher.doc(doc_address).unwrap();
    //    println!("score {score:?} doc {}", retrieved_doc.to_json(&schema));
    //}

    // use functional programming to return a vector of the document titles

    let top_docs = top_docs
        .iter()
        .map(|(_score, doc_address)| searcher.doc(*doc_address).unwrap())
        .collect::<Vec<TantivyDocument>>();

    return Json(
        top_docs
            .iter()
            .filter_map(|doc| match doc.get_first(title) {
                Some(title) => Some(title.as_str().expect("Expected a title").to_string()),
                None => None,
            })
            .collect(),
    );
}
