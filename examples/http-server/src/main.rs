extern crate actix;
extern crate actix_lua;
extern crate actix_web;
extern crate env_logger;
extern crate futures;
extern crate bytes;

use actix::prelude::*;
use actix_lua::{LuaActor, LuaActorBuilder, LuaMessage};
use actix_web::{
    middleware, server, App, AsyncResponder, FutureResponse, HttpResponse, HttpRequest, HttpMessage
};
use std::collections::HashMap;
use futures::Future;
use bytes::Bytes;

struct AppState {
    lua: Addr<LuaActor>,
}

fn build_message(req: &HttpRequest<AppState>, body: &str) -> LuaMessage {
    let mut headers = HashMap::new();

    for (key, value) in req.headers().iter() {
        let val_str = value.to_str().expect("header can't be converted to a string");
        let val_msg = LuaMessage::from(val_str);
        headers.insert(key.as_str().to_string(), val_msg);
    }

    let mut t = HashMap::new();
    t.insert("path".to_string(), LuaMessage::from(req.path()));
    t.insert("method".to_string(), LuaMessage::from(req.method().to_string()));
    t.insert("version".to_string(), LuaMessage::from(format!("{:?}", req.version())));
    t.insert("query_string".to_string(), LuaMessage::from(req.query_string()));
    t.insert("headers".to_string(), LuaMessage::from(headers));
    t.insert("body".to_string(), LuaMessage::from(body));

    LuaMessage::from(t)
}

fn index(req: HttpRequest<AppState>) -> FutureResponse<HttpResponse> {
    req.body()
        .from_err()
        .and_then(move |body: Bytes| {  // <- complete body
            let state = req.state();
            let message = build_message(&req, std::str::from_utf8(&body).expect("couldn't read body as UTF8 string"));

            state.lua.send(message).from_err()
        })
        .and_then(|res| match res {
            LuaMessage::String(s) => Ok(HttpResponse::Ok().body(s)),

            // ignore everything else
            _ => unimplemented!(),
        })
        .responder()
}

fn main() {
    ::std::env::set_var("RUST_LOG", "actix_web=info");
    env_logger::init();
    let sys = actix::System::new("actix-lua-example");

    let addr = Arbiter::start(|_| {
        LuaActorBuilder::new()
            .on_handle_with_lua(
                r#"
                    local headers = ""
                    for key,value in pairs(ctx.msg["headers"]) do
                        headers = headers .. key .. ": " .. value .. "\n"
                    end

                    local result = ctx.msg["method"] .. " " .. ctx.msg["path"] .. " " .. ctx.msg["version"] .. "\n"
                            .. "\n"
                            .. "HTTP headers:\n"
                            .. headers
                            .. "\n"
                            .. "Request body:\n"
                            .. ctx.msg["body"]

                    print(result)

                    return result
                "#,
            )
            .build()
            .unwrap()
    });

    // Start http server
    server::new(move || {
        App::with_state(AppState{lua: addr.clone()})
            // enable logger
            .middleware(middleware::Logger::default())
            .default_resource(|r| r.with_async(index))
    }).bind("127.0.0.1:8080")
        .unwrap()
        .start();

    println!("Started http server: 127.0.0.1:8080");
    let _ = sys.run();
}
