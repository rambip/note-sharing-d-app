use yew::prelude::*;
use markdown;
use std::{collections::HashMap, result};
use serde::Deserialize;
// use serde_json::{Result, Value};
use gloo_net::http::Request;
use chrono::{DateTime, Utc, serde::ts_seconds_option};
use wasm_bindgen::{JsCast, UnwrapThrowExt};
use web_sys::{Event, HtmlInputElement, InputEvent};
use regex::Regex;

#[derive(Debug, Clone)]
struct IpfsHash(String);

impl<'de> Deserialize<'de> for IpfsHash {
    fn deserialize<D>(deserializer: D) -> result::Result<Self, D::Error>
        where
            D: serde::Deserializer<'de> {
                {
                    let s = String::deserialize(deserializer)?;
                    Ok(IpfsHash(s))
                }
            }
}

/// request asking for the file with the corresponding hash of ipfs.io
fn ipfs_request(h: &IpfsHash) -> Request {
    Request::get(&format!("https://ipfs.io/ipfs/{}", h.0))
}

struct App {
    vault: Option<Vault>,
    status: Status
}

enum Status {
    Home(String),
    Error,
    WaitingForFile(String),
    WaitingForVault(String),
    Reading(Note)
}

struct Note {
    name: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct Vault {
    root: String,
    author: String,
    // TODO: better date formating 
    // https://stackoverflow.com/questions/67803619/using-serdeserialize-with-optionchronodatetime
    // #[serde(with = "ts_seconds_option")]
    // date: Option<DateTime<Utc>>,
    // TODO: faire 2 structures differentes pour la description et la table
    note_hash: HashMap<String, IpfsHash>
}

enum Msg {
    FetchVault,
    ReceiveVault(Vault),
    FetchNote(String),
    ReceiveNote(Note),
    SetUrl(String),
}


async fn fetch_note_content_and_read(note_name: String, hash: IpfsHash) -> Msg {
    let content = ipfs_request(&hash)
        .send()
        .await
        .expect("la requette ipfs a échoué")
        .text()
        .await
        .expect("contenu du fichier invalide");

    Msg::ReceiveNote(Note {name: note_name.to_string(), content})
}

async fn fetch_vault_description_and_start(url: String) -> Msg {
    let vault = Request::get(&url)
        .send()
        .await
        .expect("url non valide")
        .json()
        .await
        .expect("fichier json invalide");

    Msg::ReceiveVault(vault)
}

fn get_value_from_input_event(e: InputEvent) -> String {
    let event: Event = e.dyn_into().unwrap_throw();
    let event_target = event.target().unwrap_throw();
    let target: HtmlInputElement = event_target.dyn_into().unwrap_throw();
    web_sys::console::log_1(&target.value().into());
    target.value()
}


// https://stackoverflow.com/questions/1367409/how-to-make-button-look-like-a-link
fn markdown_to_html(content: &str, ctx: &Context<App>) -> Html {
    let raw_html = format!("<div style=\"border: 2px solid red\">{}</div>", markdown::to_html(content));
    let re = Regex::new(r"\[\[(?P<l>.*)\]\]").unwrap();
    let after = re.replace_all(&raw_html, "<button class=\"link\">$l</button>").to_string();
    Html::from_html_unchecked(AttrValue::from(after))

    // TODO
    // https://yew.rs/docs/concepts/html/lists
    // https://yew.rs/docs/concepts/html/events#event-types
}

impl Component for App {
    type Message = Msg;

    type Properties = ();

    fn create(_ctx: &Context<Self>) -> Self {
        App {status : Status::Home("enter url".to_string()), vault: None}
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::SetUrl(url) => self.status = Status::Home(url),
            Msg::FetchVault => {
                match &self.status {
                    Status::Home(url) => ctx.link().send_future(fetch_vault_description_and_start(url.clone())),
                     _ => panic!()
                }
            }
            Msg::ReceiveVault(vault) => {
                let root_note_name = vault.root.clone();
                let hash = vault.note_hash.get(&root_note_name).unwrap().clone();
                ctx.link().send_future(fetch_note_content_and_read(root_note_name, hash));
                self.status = Status::WaitingForFile(vault.root.clone());
                self.vault = Some(vault);
            }
            Msg::ReceiveNote(note) => self.status = Status::Reading(note),
            Msg::FetchNote(_) => todo!(),
        }
        true
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let link = ctx.link().clone();
        let page = match &self.status {
            Status::Home(url) => html!{<>
                <h1> {"hello"} </h1>
                    <input type="text" value={url.clone()} oninput={link.callback(move |e: InputEvent| Msg::SetUrl(get_value_from_input_event(e)))} /> 
                    <button onclick={link.callback(move |_| Msg::FetchVault)}> {"valider"} </button>
                    </>
            },
            Status::Error => html!{<h1> {"error"} </h1>},
            Status::WaitingForFile(_) => html!{
                <p>{"the note is coming ..."}</p>
            },
            Status::WaitingForVault(_) => todo!(),
            Status::Reading(s) => html! {
                <p style="border: 2px solid red">
                {markdown_to_html(&s.content, ctx)}
                </p>
            }
        };
        html! {
            <>
            {page} 
            <h3>{"debug:"} </h3>
            <p>{format!("{:?}", self.vault)}</p>
            </>
        }
    }

    fn changed(&mut self, ctx: &Context<Self>, _old_props: &Self::Properties) -> bool {
        true
    }

    fn rendered(&mut self, ctx: &Context<Self>, first_render: bool) {}

    fn prepare_state(&self) -> Option<String> {
        None
    }

    fn destroy(&mut self, ctx: &Context<Self>) {}
}

fn main() {
    yew::Renderer::<App>::new().render();
}
