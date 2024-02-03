// This macro use makes sure that the rocket is imported globally:
#[macro_use] extern crate rocket;

use rocket::{fs::{relative, FileServer}, response::stream::{EventStream, Event}, serde::{Deserialize, Serialize}, tokio::sync::broadcast::{channel, Sender, error::RecvError}, Shutdown, State};
use rocket::form::Form;
use rocket::tokio::select;

#[derive(Debug, Clone, FromForm, Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
struct Message {
    #[field(validate = len(..30))]
    pub room: String,
    #[field(validate = len(..20))]
    pub username: String,
    pub message : String,
}

// This is a route attribute, which describes what type of request this route handles. The handler function then
// proceds to declare how that route should be processed.
// 1. This specific route is a get, through the events path.
// 2. The return type is an infinite stream of server side events which allows for a long lived connection from the client to
//    the server so that the server can send data to the clients whenever it wants. This is similar to websocket, but it only
//    works in one direction. So through this connection, the client is only allowed to receive data from the server and not
//    vice versa. 
#[get("/events")]
async fn events(queue: &State<Sender<Message>>, mut end: Shutdown) -> EventStream![] {
    
    // This codes creates a new receiver that can listen for new messages from the server:
    let mut rx = queue.subscribe();

    // Next we use "generator syntax" to yield an infinite series of server sent events. 
    // It consists of an infinite loop and firstly utilize the select macro. The select macro
    // waits for multiple concurrent branches and returns as soon as one of them completes.
    // Here we only have two branches, the first one is calling receive on our receiver which
    // waits for new messages. When we get a new message, it gets mapped to the msg variable and gets
    // matched against that. This in turn will return a result enum, if it is Ok(msg) we simply return
    // the message inside of it. If the enum Err::Closed is return, then it means there are no more senders
    // so we can break out of the infinite loop. Thee Err::Lagged means the receiver was to far behind and
    // will be forceibly disconnected.
    EventStream! {
        loop {
            let msg = select! {
                msg = rx.recv() => match msg {
                    Ok(msg) => msg,
                    Err(RecvError::Closed) => break,
                    Err(RecvError::Lagged(_)) => continue,
                },
                _ = &mut end => break,
            };
            yield Event::json(&msg);
        }
    }
}

// This post endpoint receives data as form data. This will then be converted to Form data of type Message struct and the server state.
#[post("/message", data = "<form>")]
fn post(form: Form<Message>, queue: &State<Sender<Message>>) {
    // A send 'fails' if there are no active subscribers. 
    let _res = queue.send(form.into_inner());
}
// The lauch attribute will take this code and generate a main function that serves as the entry point of the application and
// start the rocket server.
#[launch]
fn rocket() -> _ {
    rocket::build()
        .manage(channel::<Message>(1024).0) // This adds the possibility of state that all handlers will have access to. In this case we are adding a channel(sender)
        .mount("/", routes![post, events]) // Here the last parameter is an array of possible handler functions. Namely mounting post and events to the root path
        .mount("/", FileServer::from(relative!("static"))) // Here we implement a file server that will handle static files (normally html and css files)
}