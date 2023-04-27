use yew::prelude::*;
use markdown;
use std::{collections::HashMap, result};
use serde::Deserialize;
use gloo_net::http::Request;
use gloo::events::EventListener;
use wasm_bindgen::{JsCast, UnwrapThrowExt};
use web_sys::{Event, HtmlInputElement, HtmlElement, HtmlButtonElement, InputEvent};
use regex::Regex;
use log::info;

#[derive(Debug, Clone)]
struct IpfsHash(String);

impl<'de> Deserialize<'de> for IpfsHash {
    fn deserialize<D>(deserializer: D) -> result::Result<Self, D::Error>
        where
        D: serde::Deserializer<'de> {
            let s = String::deserialize(deserializer)?;
            Ok(IpfsHash(s))
        }
}


struct App {
    vault: Option<Vault>,
    markdown_view: NodeRef,
    link_listeners: Vec<EventListener>,
    status: Status
}

enum Status {
    Home(String),
    Error,
    WaitingForFile(String),
    WaitingForVault(String),
    Reading(String)
}


/// representation of a vault, a set of markdown notes.
/// It can be serialized and deserialized using json.
/// `vault.ipfsmap` is a HashMap that maps local links to
/// ipfs content identifiers
#[derive(Debug, Deserialize)]
struct Vault {
    root: String,
    author: String,
    // TODO: date 
    // https://stackoverflow.com/questions/67803619/using-serdeserialize-with-optionchronodatetime
    ipfsmap: HashMap<String, IpfsHash>
}

enum Msg {
    FetchVault,
    ReceiveVault(Vault),
    FetchNote(IpfsHash),
    ReceiveNote(String),
    SetUrl(String),
}

/// request a file using its cid on ipfs.io
/// If the client has installed [ipfs](ipfs.io), it will not use the gateway
fn ipfs_request(h: &IpfsHash) -> Request {
    Request::get(&format!("https://ipfs.io/ipfs/{}", h.0))
}

async fn fetch_note_content_and_read(hash: IpfsHash) -> Msg {
    let content = ipfs_request(&hash)
        .send()
        .await
        .expect("la requette ipfs a échoué")
        .text()
        .await
        .expect("contenu du fichier invalide");

    Msg::ReceiveNote(content)
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



/// `extract_link(wikilink, associations)` extracts from `wikilink` of the form `link|text` 
/// a couple `(text, ipfs_link)` where
/// - `text` is the textual part of the link
/// - `ipfs_link` is the hash associated to the address part of the link
fn extract_link(wikilink: &str, associations: &HashMap<String, IpfsHash>) -> (String, Option<IpfsHash>)  {
    let parts_of_link : Vec<&str> = wikilink.split("|").collect();
    if parts_of_link.len() == 2 {
        (parts_of_link[1].to_string(), associations.get(parts_of_link[0]).map(|x| x.clone()))
    }
    else {
        info!("{}", parts_of_link[0]);
        (parts_of_link[0].to_string(), associations.get(parts_of_link[0]).map(|x| x.clone()))
    }
}

/// `set_markdown_content(content, associations, html_element, ctx)` change the element 
/// of the node `html_element` with the html representation of the markdown `content`.
/// It also converts all the \[\[wikilinks\]\] from the markdown to clickable buttons,
/// using `associations` to create the ipfs links.
/// It will return a list of `EventListener` corresponding to the button click-events
fn set_markdown_content(content: &str, associations: &HashMap<String, IpfsHash>, 
                        html_element: &HtmlElement, ctx: &Context<App>, listeners: &mut Vec<EventListener>) {
    // TODO: styling. https://stackoverflow.com/questions/1367409/how-to-make-button-look-like-a-link

    let raw_html = format!("<div style=\"border: 2px solid red\">{}</div>", markdown::to_html(content));
    let re = Regex::new(r"\[\[(.*?)\]\]").unwrap();
    let html_with_link_converted = re.replace_all(&raw_html, "<button></button>").to_string();
    let link_matches : Vec<_> = re.captures_iter(&raw_html).collect();

    html_element.set_inner_html(&html_with_link_converted);

    let links = html_element.query_selector_all("button").unwrap();
    listeners.clear();
    for i in 0..link_matches.len() {
        let button: HtmlButtonElement = links.get(i as u32).unwrap().dyn_into().unwrap();
        let link_text = &link_matches[i][1];
        let (name, hash) = extract_link(&link_text, associations);
        button.set_inner_text(&name);
        if let Some(real_hash) = hash {
            // lien disponible
            let callback = ctx.link().callback(move |()| Msg::FetchNote(real_hash.clone()));
            let event_listener = EventListener::new(&button, "click", move |_| callback.emit(()));
            listeners.push(event_listener);
        }
        else {
            // lien non disponible
            button.style().set_property("background-color", "red").unwrap()
        }
    }
}

fn get_value_from_input_event(e: InputEvent) -> String {
    let event: Event = e.dyn_into().unwrap_throw();
    let event_target = event.target().unwrap_throw();
    let target: HtmlInputElement = event_target.dyn_into().unwrap_throw();
    target.value()
}

/// input element to enter url of a vault
fn url_input(url: &str, ctx: &Context<App>) -> Html {
    let oninput = ctx.link().
        callback(move |e: InputEvent| Msg::SetUrl(get_value_from_input_event(e)));

    html!{
        <input type="text" value={url.to_string()} oninput={oninput} /> 
    }
}

impl Component for App {
    type Message = Msg;

    type Properties = ();

    fn create(_ctx: &Context<Self>) -> Self {
        App {
            status : Status::Home("enter url".to_string()),
            vault: None,
            markdown_view: NodeRef::default(),
            link_listeners: vec![],
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::SetUrl(url) => self.status = Status::Home(url),
            Msg::FetchVault => {
                match &self.status {
                    Status::Home(url) => {
                        ctx.link().send_future(fetch_vault_description_and_start(url.clone()));
                    },
                    _ => panic!()
                }
            }
            Msg::ReceiveVault(vault) => {
                let root_note_name = vault.root.clone();
                let hash = vault.ipfsmap.get(&root_note_name).unwrap().clone();
                ctx.link().send_future(fetch_note_content_and_read(hash));
                self.status = Status::WaitingForFile(vault.root.clone());
                self.vault = Some(vault);
            }
            Msg::ReceiveNote(content) => {
                set_markdown_content(&content, 
                                     &self.vault.as_ref().unwrap().ipfsmap, 
                                     &self.markdown_view.cast::<HtmlElement>().unwrap(), 
                                     ctx,
                                     &mut self.link_listeners
                );
                self.status = Status::Reading(content);
            }
            Msg::FetchNote(hash) => {
                ctx.link().send_future(fetch_note_content_and_read(hash));
            }
        }
        true
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        let link = ctx.link().clone();
        let page = match &self.status {
            Status::Home(url) => html!{<>
                <h1> {"hello"} </h1>
                    {url_input(url, ctx)}
                    <button onclick={link.callback(move |_| Msg::FetchVault)}> {"valider"} </button>
                    </>
            },
            Status::Error => html!{<h1> {"error"} </h1>},
            Status::WaitingForFile(_) => html!{
                <p>{"the note is coming ..."}</p>
            },
            Status::WaitingForVault(_) => todo!(),
            Status::Reading(s) => html! {
                "reading ..."
            }
        };
        html! {
            <>
            {page} 
            <div style="border: 2px solid red" ref={&self.markdown_view}> </div>
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
    wasm_logger::init(wasm_logger::Config::default());
    yew::Renderer::<App>::new().render();
}
