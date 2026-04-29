use gtk4::prelude::*;
use gtk4::{Button, Label, glib};
use serde::Deserialize;
use std::time::Duration;

const TOKEN: &str = "eede2aac49a4b420091a181c837a32f7609022dc";
const CITY: &str = "Murcia";
const UPDATE_INTERVAL_MINS: u64 = 30; // AQI does not change fast.

#[derive(Deserialize)]
struct WaqiResponse {
    status: String,
    data: Option<WaqiData>,
}

#[derive(Deserialize)]
struct WaqiData {
    aqi: i32,
}

#[derive(Deserialize)]
struct MozillaLoc {
    location: LatLng,
}

#[derive(Deserialize)]
struct LatLng {
    lat: f64,
    lng: f64,
}

fn get_icon(aqi: i32) -> &'static str {
    match aqi {
        0..=99 => "",
        100..=149 => "",
        150..=199 => "",
        200..=299 => "",
        _ => "",
    }
}

async fn fetch_aqi() -> String {
    let client = reqwest::Client::new();

    // 1. Determine URL (City vs Geolocation)
    let url = if !CITY.is_empty() {
        format!("https://api.waqi.info/feed/{}/?token={}", CITY, TOKEN)
    } else {
        let loc_req = client
            .get("https://location.services.mozilla.com/v1/geolocate?key=geoclue")
            .send()
            .await;

        if let Ok(resp) = loc_req {
            if let Ok(loc) = resp.json::<MozillaLoc>().await {
                format!(
                    "https://api.waqi.info/feed/geo:{};{}/?token={}",
                    loc.location.lat, loc.location.lng, TOKEN
                )
            } else {
                return " LocErr".to_string();
            }
        } else {
            return " NetErr".to_string();
        }
    };

    // 2. Fetch AQI
    if let Ok(resp) = client.get(url).send().await
        && let Ok(json) = resp.json::<WaqiResponse>().await
        && json.status == "ok"
        && let Some(data) = json.data
    {
        return format!("{}  {}", get_icon(data.aqi), data.aqi);
    }

    " Error".to_string()
}

pub fn init(container: &gtk4::Box) {
    let label = Label::builder().label(" ...").build();
    label.set_overflow(gtk4::Overflow::Visible);
    label.set_margin_start(1);

    let button = Button::new();
    button.add_css_class("btn");
    button.set_overflow(gtk4::Overflow::Visible);
    button.set_child(Some(&label));
    container.append(&button);

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();

    // UI Listener: Receives updates from the background thread
    let label = label.clone();
    glib::MainContext::default().spawn_local(async move {
        while let Some(text) = rx.recv().await {
            label.set_label(&text);
        }
    });

    // Background Worker: Handles networking and timing
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async move {
            loop {
                let display_text = fetch_aqi().await;
                let _ = tx.send(display_text);

                // Sleep for the interval
                tokio::time::sleep(Duration::from_secs(UPDATE_INTERVAL_MINS * 60)).await;
            }
        });
    });
}
