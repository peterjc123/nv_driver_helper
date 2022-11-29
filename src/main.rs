#![windows_subsystem = "windows"]

use fltk::{app, prelude::*, *};
use fltk_theme::{ThemeType, WidgetTheme};
use serde::{Deserialize, Serialize};
use std::cmp;
use std::collections::HashMap;
use std::os::windows::process::CommandExt;
use version_compare::Version;
use winapi::um::winbase::{CREATE_NO_WINDOW, DETACHED_PROCESS};

#[derive(Debug, Deserialize)]
struct ProductRecord {
    pt: String,
    pst: String,
    pf: String,
    ptid: String,
    pstid: String,
    pfid: String,
}

#[derive(Debug, Deserialize)]
struct LangRecord {
    langid: String,
    lang: String,
}

#[derive(Debug, Deserialize)]
struct OSRecord {
    osid: String,
    os: String,
}

#[derive(Debug, Deserialize)]
struct TypeRecord {
    typeid: String,
    typeval: String,
}

struct ProductData {
    pt: Vec<(String, String)>,
    pst: HashMap<i32, Vec<(String, String)>>,
    pf: HashMap<(i32, i32), Vec<(String, String)>>,
}

#[derive(Debug, Clone, Copy)]
pub enum Message {
    Refresh(usize),
    Click(usize),
    Repaint(usize),
    Query,
}

#[derive(Debug, Clone)]
pub enum EMessage {
    ShowDialog(String),
    ShowChoice(String, String),
}

#[derive(Serialize, Deserialize)]
struct Config {
    selected_items: Vec<i32>,
}

impl ::std::default::Default for Config {
    fn default() -> Self {
        Self {
            selected_items: vec![1, 0, 0, -1, -1, 1],
        }
    }
}

async fn prepare_lang_data() -> Result<Vec<(String, String)>, Box<dyn std::error::Error>> {
    let mut res: Vec<(String, String)> = Vec::default();

    let data = include_str!("../data/lang_mapping.csv");

    let mut rdr = csv::Reader::from_reader(data.as_bytes());
    for result in rdr.deserialize() {
        let record: LangRecord = result?;
        println!("{:?}", record);
        res.push((record.lang, record.langid));
    }

    Ok(res)
}

async fn prepare_os_data() -> Result<Vec<(String, String)>, Box<dyn std::error::Error>> {
    let mut res: Vec<(String, String)> = Vec::default();

    let data = include_str!("../data/os_mapping.csv");

    let mut rdr = csv::Reader::from_reader(data.as_bytes());
    for result in rdr.deserialize() {
        let record: OSRecord = result?;
        println!("{:?}", record);
        res.push((record.os, record.osid));
    }

    Ok(res)
}

async fn prepare_type_data() -> Result<Vec<(String, String)>, Box<dyn std::error::Error>> {
    let mut res: Vec<(String, String)> = Vec::default();

    let data = include_str!("../data/type_mapping.csv");

    let mut rdr = csv::Reader::from_reader(data.as_bytes());
    for result in rdr.deserialize() {
        let record: TypeRecord = result?;
        println!("{:?}", record);
        res.push((record.typeval, record.typeid));
    }

    Ok(res)
}

async fn prepare_product_data() -> Result<ProductData, Box<dyn std::error::Error>> {
    let data = include_str!("../data/product_mapping.csv");

    let mut rdr = csv::Reader::from_reader(data.as_bytes());
    let mut pt_idx = -1;
    let mut pst_idx = 0;
    let dummy_value = String::default();
    let mut prev_pt: &String = &dummy_value;
    let mut prev_pst: &String = &dummy_value;
    let mut c: Vec<(String, String)> = Vec::default();
    let mut b: HashMap<i32, Vec<(String, String)>> = HashMap::default();
    let mut a: HashMap<(i32, i32), Vec<(String, String)>> = HashMap::default();
    for result in rdr.deserialize() {
        let record: ProductRecord = result?;
        if &record.pt != prev_pt {
            pt_idx += 1;
            pst_idx = 0;
            println!("{}, {}, {:?}", pt_idx, pst_idx, record);
            c.push((record.pt, record.ptid));
            b.insert(pt_idx, vec![(record.pst, record.pstid)]);
            a.insert((pt_idx, pst_idx), vec![(record.pf, record.pfid)]);
        } else if &record.pst != prev_pst {
            pst_idx += 1;
            println!("{}, {}, {:?}", pt_idx, pst_idx, record);
            b.get_mut(&pt_idx).unwrap().push((record.pst, record.pstid));
            a.insert((pt_idx, pst_idx), vec![(record.pf, record.pfid)]);
        } else {
            println!("{}, {}, {:?}", pt_idx, pst_idx, record);
            a.get_mut(&(pt_idx, pst_idx))
                .unwrap()
                .push((record.pf, record.pfid));
        }

        prev_pt = &c.last().unwrap().0;
        prev_pst = &b.get(&pt_idx).unwrap().last().unwrap().0;
    }
    Ok(ProductData {
        pt: c,
        pst: b,
        pf: a,
    })
}

async fn get_current_gpu_version() -> Result<String, Box<dyn std::error::Error>> {
    let out = std::process::Command::new("nvidia-smi")
        .arg("--query-gpu=driver_version")
        .arg("--format=csv")
        .stdout(std::process::Stdio::piped())
        .creation_flags(CREATE_NO_WINDOW)
        .output()?;

    let raw_out = String::from_utf8(out.stdout)?;
    let outs: Vec<&str> = raw_out.split('\n').collect();

    let out = outs[1];

    let out = if out.ends_with('\r') {
        out.strip_suffix('\r').unwrap()
    } else {
        out
    };

    Ok(String::from(out))
}

async fn get_latest_gpu_version(url: &str) -> Result<(String, String), Box<dyn std::error::Error>> {
    let result = get_latest_version_info(url).await?;

    let items: &Vec<serde_json::Value> = result["IDS"].as_array().unwrap();
    if let Some(item) = items.iter().next() {
        let download_info = &item["downloadInfo"];
        let v = download_info["Version"].as_str().unwrap();
        let d = download_info["DownloadURL"].as_str().unwrap();
        println!("{}", d);
        return Ok((String::from(v), String::from(d)));
    }
    Err("".into())
}

async fn get_latest_version_info(url: &str) -> Result<serde_json::Value, reqwest::Error> {
    reqwest::Client::new().get(url).send().await?.json().await
}

async fn get_region_html() -> Result<String, reqwest::Error> {
    reqwest::Client::new()
        .get("https://check-host.net/ip/widget.js")
        .send()
        .await?
        .text()
        .await
}

async fn get_region() -> String {
    let region_text = get_region_html().await.unwrap_or_else(|_| "".into());
    region_text.find("alt=\\\"").map_or("".into(), |i| {
        let region = &region_text[i + 6..i + 8];
        String::from(region)
    })
}

fn load_config(selected_items: &mut Vec<i32>) -> Result<(), confy::ConfyError> {
    let cfg: Config = confy::load("nv_driver_helper")?;
    for (i, s) in selected_items.iter_mut().enumerate() {
        *s = *cfg.selected_items.get(i).unwrap();
    }
    Ok(())
}

fn save_config(selected_items: &[i32]) -> Result<(), confy::ConfyError> {
    let mut cfg = Config {
        selected_items: Vec::with_capacity(selected_items.len()),
    };
    for s in selected_items {
        cfg.selected_items.push(*s);
    }
    confy::store("nv_driver_helper", cfg)
}

#[tokio::main]
async fn main() {
    let app = app::App::default();

    let widget_theme = WidgetTheme::new(ThemeType::Greybird);
    widget_theme.apply();

    let mut my_window = window::Window::default().with_size(500, 300);
    my_window.set_label("Nvidia Driver Download Helper");

    let data = include_bytes!("../res/icon.png");
    let img = fltk::image::PngImage::from_data(data).ok();
    my_window.set_icon(img);

    let mut hpack = group::Pack::new(150, 30, 300, 200, "");
    hpack.set_type(group::PackType::Vertical);
    hpack.set_spacing(10);

    let mut choices = vec![
        menu::Choice::default().with_label("Product Type"),
        menu::Choice::default().with_label("Product Series"),
        menu::Choice::default().with_label("Product"),
        menu::Choice::default().with_label("Operation System"),
        menu::Choice::default().with_label("Language"),
        menu::Choice::default().with_label("Download Type"),
    ];

    hpack.end();

    hpack.auto_layout();

    let mut hpack_1 = group::Pack::new(200, 250, 100, 20, "");
    hpack_1.set_type(group::PackType::Vertical);
    hpack_1.set_spacing(10);

    let mut btn = button::Button::default().with_label("Query");

    hpack_1.end();
    hpack_1.auto_layout();

    my_window.end();
    my_window.show();

    let product_data = prepare_product_data()
        .await
        .expect("Failed to unpack product data");

    let lang_data = prepare_lang_data()
        .await
        .expect("Failed to unpack lang data");

    let os_data = prepare_os_data().await.expect("Failed to unpack os data");

    let type_data = prepare_type_data()
        .await
        .expect("Failed to unpack type data");

    let mut selected_items = vec![1, 0, 0, -1, -1, 1];
    load_config(&mut selected_items)
        .unwrap_or_else(|e| println!("Cannot load config due to {:?}", e));

    let first_idx = selected_items.get(0).unwrap();
    let second_idx = selected_items.get(1).unwrap();

    let mut global_dict: Vec<&Vec<(String, String)>> = vec![
        &product_data.pt,
        product_data.pst.get(first_idx).unwrap(),
        product_data.pf.get(&(*first_idx, *second_idx)).unwrap(),
        &os_data,
        &lang_data,
        &type_data,
    ];

    let (s, r) = fltk::app::channel::<Message>();

    for (i, v) in global_dict.iter().enumerate() {
        let choice = choices.get_mut(i).unwrap();

        for (k, _) in *v {
            choice.add_choice(k.as_str());
        }

        if !v.is_empty() {
            let selected = selected_items.get_mut(i).unwrap();
            *selected = cmp::max(*selected, 0);
            s.send(Message::Repaint(i));
        }

        choice.set_callback(move |x| {
            if x.changed() {
                s.send(Message::Click(i));
            }
        });
    }

    btn.set_callback(move |_| {
        s.send(Message::Query);
    });

    let (ss, rr) = std::sync::mpsc::channel::<EMessage>();

    while app.wait() {
        if let Some(msg) = r.recv() {
            match msg {
                Message::Click(i) => {
                    let selected = selected_items.get_mut(i).unwrap();
                    let idx = choices.get(i).unwrap().value();
                    *selected = idx;
                    println!("Click: Control Idx #{}, Value Idx #{}", i, idx);
                    if i < 2 {
                        s.send(Message::Refresh(i));
                    }
                }
                Message::Refresh(i) => {
                    println!("Refresh: Control Idx #{}", i);
                    match i {
                        0 => {
                            let pst_selected = selected_items.get_mut(i + 1).unwrap();
                            *pst_selected = 0;
                            s.send(Message::Refresh(i + 1));
                        }
                        1 => {
                            {
                                let pf_selected = selected_items.get_mut(i + 1).unwrap();
                                *pf_selected = 0;
                            }

                            let pt_selected = selected_items.get(i - 1).unwrap();
                            let pst_selected = selected_items.get(i).unwrap();

                            *global_dict.get_mut(i).unwrap() =
                                product_data.pst.get(pt_selected).unwrap();
                            *global_dict.get_mut(i + 1).unwrap() =
                                product_data.pf.get(&(*pt_selected, *pst_selected)).unwrap();

                            for i in &[i, i + 1] {
                                let choice = choices.get_mut(*i).unwrap();
                                choice.clear();

                                for (k, _) in *global_dict.get(*i).unwrap() {
                                    choice.add_choice(k.as_str());
                                }

                                s.send(Message::Repaint(*i));
                            }
                        }
                        _ => {}
                    }
                }
                Message::Repaint(i) => {
                    println!("Repaint: Control Idx #{}", i);
                    let selected = selected_items.get_mut(i).unwrap();
                    let choice = choices.get_mut(i).unwrap();
                    let idx = *selected;
                    choice.set_value(idx);
                }
                Message::Query => {
                    btn.deactivate();
                    btn.set_label("Querying...");

                    let ids: Vec<&String> = selected_items
                        .iter()
                        .skip(1)
                        .enumerate()
                        .map(|(i, selected)| {
                            let idx: usize = (*selected).try_into().unwrap();
                            &global_dict.get(i + 1).unwrap().get(idx).unwrap().1
                        })
                        .collect();

                    let (psid, pfid, osid, langid, typeid) =
                        (ids[0], ids[1], ids[2], ids[3], ids[4]);

                    println!(
                        "Query: {}, {}, {}, {}, {}",
                        psid, pfid, osid, langid, typeid
                    );

                    let region = get_region().await;

                    let suffix = match region.as_str() {
                        "CN" => "cn",
                        _ => "com",
                    };

                    let url = format!("https://gfwsl.geforce.{}/services_toolkit/services/com/nvidia/services/\
                                              AjaxDriverService.php?func=DriverManualLookup&psid={}&pfid={}&osID={}&la\
                                              nguageCode={}&beta=0&isWHQL=0&dltype=-1&dch=1&upCRD={}&qnf=0&sort1=0&nu\
                                              mberOfResults=100", suffix, psid, pfid, osid, langid, typeid);

                    println!("{}", url);

                    let sender = ss.clone();

                    tokio::spawn(async move {
                        let current_version = get_current_gpu_version().await.ok();

                        if current_version.is_none() {
                            let message = String::from("Cannot retrieve current GPU version");
                            sender.send(EMessage::ShowDialog(message)).unwrap();
                            app::awake();
                            return;
                        }

                        let current_version = current_version.unwrap();

                        let latest_version = get_latest_gpu_version(&url).await.ok();

                        match latest_version {
                            Some((x, y)) => {
                                let message = format!(
                                    "Current version: {}\nLatest version: {}\n",
                                    current_version, x
                                );
                                if Version::from(&x).unwrap()
                                    > Version::from(&current_version).unwrap()
                                {
                                    let message = format!("{}A new update is available", message);
                                    sender.send(EMessage::ShowChoice(message, y)).unwrap();
                                } else {
                                    let message = format!("{}No update", message);
                                    sender.send(EMessage::ShowDialog(message)).unwrap();
                                }
                            }
                            None => {
                                let message = String::from("Cannot retrieve latest GPU version");
                                sender.send(EMessage::ShowDialog(message)).unwrap();
                            }
                        }
                        app::awake();
                    });
                }
            }
        }
        if let Ok(msg) = rr.try_recv() {
            match msg {
                EMessage::ShowDialog(s) => {
                    dialog::alert_default(&s);
                }
                EMessage::ShowChoice(s, d) => {
                    let res = dialog::choice_default(&s, "Download", "Cancel", "");
                    if res == 0 {
                        std::process::Command::new("cmd.exe")
                            .arg("/C")
                            .arg("start")
                            .arg("")
                            .arg("%NV_DRIVER_DOWNLOAD_URL%")
                            .env("NV_DRIVER_DOWNLOAD_URL", format!("\"{}\"", d).as_str())
                            .creation_flags(DETACHED_PROCESS)
                            .spawn()
                            .expect("failed to launch browser");
                    }
                }
            }
            btn.set_label("Query");
            btn.activate();
        }
    }

    save_config(&selected_items).unwrap_or_else(|e| println!("Cannot save config due to {:?}", e));
}
