#![deny(warnings)]
extern crate hyper;
extern crate hyper_tls;
extern crate tokio;

use hyper::{Server, Body, Client, Method, StatusCode, Request, Response};
use hyper::service::{make_service_fn, service_fn};
use hyper_tls::HttpsConnector;
use hyper::client::HttpConnector;

use std::env;

type GenericError = Box<dyn std::error::Error + Send + Sync>;
type Result<T> = std::result::Result<T, GenericError>;

static NOTFOUND: &[u8] = b"Not Found";
static OK: &[u8] = b"OK";
static REQ_ERR: &[u8] = b"request error";


async fn handle_request (
    req: Request<Body>,
    client: Client<HttpsConnector<HttpConnector>>,
    urls: Vec<String>,
) -> Result<Response<Body>> {
        match (req.method(), req.uri().path()) {
            (&Method::GET, "/ip") => {
                let query = req.uri().query().unwrap_or("");
                let ip = query.split('&').find(|x| x.starts_with("myip=")).unwrap_or("").replace("myip=", "");
                // Run a web query against the web api below
                for url in &urls {
                    let resp = client.get(url.replace("$ip", &ip).parse()?).await?;
                    if !resp.status().is_success() {
                        return Ok(Response::builder().status(StatusCode::BAD_GATEWAY).body(Body::from(REQ_ERR)).unwrap())
                    }
                }
                Ok(Response::builder().body(Body::from(OK)).unwrap())
            },
            _ => {
                Ok(Response::builder().body(Body::from(NOTFOUND)).unwrap())
            }
        }
    }

fn get_urls() -> Vec<String> {
	let mut urls = Vec::new();
    for i in 0..9 {
    	let mut key = String::from("URL");
		key.push(std::char::from_digit(i, 10).unwrap());
		match env::var_os(key) {
    		Some(val) => urls.push(val.into_string().unwrap()),
    		None => ()
		}
	}
	return urls;
}

#[tokio::main]
async fn main() -> Result<()> {
    let addr = "0.0.0.0:1337".parse().unwrap();
    let https = HttpsConnector::new();
    let client = Client::builder().build::<_, hyper::Body>(https);
    let new_service = make_service_fn(move |_| {
        // Move a clone of `client` into the `service_fn`.
        let client = client.clone();
        let urls = get_urls();
        async {
            Ok::<_, GenericError>(service_fn(move |req| {
                // Clone again to ensure that client outlives this closure.
                handle_request(req, client.to_owned(), urls.to_owned())
            }))
        }
    });

    let server = Server::bind(&addr).serve(new_service);
    println!("Listening on http://{} with 1 thread.", addr);
    server.await?;
    Ok(())
}
