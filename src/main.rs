#![deny(warnings)]
extern crate futures;
extern crate hyper;
extern crate tokio_core;

use futures::{Future, Stream};

use hyper::{Body, Chunk, Client, Get, StatusCode};
use hyper::error::Error;
use hyper::header::ContentLength;
use hyper::server::{Http, Service, Request, Response};

use std::env;

static NOTFOUND: &[u8] = b"Not Found";
static OK: &[u8] = b"OK";
static REQ_ERR: &[u8] = b"request error";

pub type ResponseStream = Box<Stream<Item=Chunk, Error=Error>>;

struct DynDnsProxy(tokio_core::reactor::Handle);

impl Service for DynDnsProxy {
    type Request = Request;
    type Response = Response<ResponseStream>;
    type Error = hyper::Error;
    type Future = Box<Future<Item = Self::Response, Error = Self::Error>>;

    fn call(&self, req: Request) -> Self::Future {
        match (req.method(), req.path()) {
            (&Get, "/ip") => {
                let urls = get_urls() ;
                let query = req.query().unwrap_or("");
                let ip = query.split('&').find(|x| x.starts_with("ip=")).unwrap_or("").replace("ip=", "");
                // Run a web query against the web api below
                let client = Client::configure().build(&self.0);
                let mut future : Box<Future<Item=_,Error=_>> = Box::new(futures::future::ok(Response::new()));
                for url in &urls {
                    let req = Request::new(Get, url.replace("$ip", ip).parse().unwrap());
                    let web_res_future : Box<Future<Item=Response,Error=_>> = Box::new(client.request(req));
                    future = Box::new(future.and_then(|r| if r.status().is_success() {web_res_future} else {Box::new(futures::future::err(Error::Status))}));
                }
				Box::new(future.map(|_| {
                	let body: ResponseStream = Box::new(Body::from(OK));
                    Response::new()
                        .with_body(body)
                        .with_header(ContentLength(OK.len() as u64))
                }).or_else(|_| {
                	let body: ResponseStream = Box::new(Body::from(REQ_ERR));
                    futures::future::ok(Response::new()
                        .with_body(body)
                        .with_status(StatusCode::BadGateway)
                        .with_header(ContentLength(REQ_ERR.len() as u64)))
                }))
            },
            _ => {
                let body: ResponseStream = Box::new(Body::from(NOTFOUND));
                Box::new(futures::future::ok(Response::new()
                                             .with_status(StatusCode::NotFound)
                                             .with_header(ContentLength(NOTFOUND.len() as u64))
                                             .with_body(body)))
            }
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


fn main() {
    let addr = "127.0.0.1:1337".parse().unwrap();

    let mut core = tokio_core::reactor::Core::new().unwrap();
    let handle = core.handle();
    let client_handle = core.handle();

    let serve = Http::new().serve_addr_handle(&addr, &handle, move || Ok(DynDnsProxy(client_handle.clone()))).unwrap();
    println!("Listening on http://{} with 1 thread.", serve.incoming_ref().local_addr());

    let h2 = handle.clone();
    handle.spawn(serve.for_each(move |conn| {
        h2.spawn(conn.map(|_| ()).map_err(|err| println!("serve error: {:?}", err)));
        Ok(())
    }).map_err(|_| ()));

    core.run(futures::future::empty::<(), ()>()).unwrap();
}
