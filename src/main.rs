use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{HtmlCanvasElement, HtmlVideoElement, MediaStream, FormData};
use yew::prelude::*;
use gloo_net::http::Request;
use wasm_bindgen_futures::spawn_local;
use js_sys::Reflect;
use serde_json::json;
use gloo_utils::format::JsValueSerdeExt;
use serde::Deserialize;



#[derive(Deserialize)]
struct Detections {
    box_: Vec<Vec<f32>>, // Change to a nested list of floats
    confidence: f32,
    label: String,
}

#[derive(Deserialize)]
struct DetectionResponse {
    detections: Vec<Detections>,
    url: String,
}


#[function_component(App)]
fn app() -> Html {
    let video_ref = use_node_ref();
    let canvas_ref = use_node_ref();
    let processed_image_url = use_state::<Option<String>, _>(|| None);
    let detections = use_state::<Vec<Detections>, _>(|| Vec::new());
    let total_count = use_state(|| 0.0);
    let show_count = use_state(|| false);
    let combo_result = use_state::<Option<String>, _>(|| None); 

    let request_camera = {
        let video_ref = video_ref.clone();
        Callback::from(move |_| {
            let video_ref = video_ref.clone();
            let window = web_sys::window().unwrap();
            let navigator = window.navigator();
            let media_devices = navigator.media_devices().unwrap();

            let constraints = JsValue::from_serde(&json!({
                "video": true
            })).unwrap();

            let promise = Reflect::get(&media_devices, &JsValue::from_str("getUserMedia"))
                .unwrap()
                .dyn_into::<js_sys::Function>()
                .unwrap()
                .call1(&media_devices, &constraints)
                .unwrap();

            let future = wasm_bindgen_futures::JsFuture::from(js_sys::Promise::from(promise));

            spawn_local(async move {
                match future.await {
                    Ok(stream) => {
                        let video_element = video_ref.cast::<HtmlVideoElement>().unwrap();
                        let media_stream = stream.dyn_into::<MediaStream>().unwrap();
                        video_element.set_src_object(Some(&media_stream));
                        video_element.set_autoplay(true);
                    },
                    Err(err) => {
                        web_sys::console::log_1(&JsValue::from(format!("Error accessing camera: {:?}", err)));
                    }
                }
            });
        })
    };

let capture_image = {
    let video_ref = video_ref.clone();
    let canvas_ref = canvas_ref.clone();
    let processed_image_url = processed_image_url.clone();
    let detections = detections.clone(); // Clone the `detections` state here

    Callback::from(move |_| {
        if let Some(url) = (*processed_image_url).clone() {
            if let Some(filename) = url.split('/').last().map(String::from) {
                let delete_url = format!("http://localhost:8000/delete-image?filename={}", filename);
                
                // Debugging: Print the delete URL
                web_sys::console::log_1(&JsValue::from_str(&format!("Attempting to delete image at URL: {}", delete_url)));
                
                spawn_local(async move {
                    match Request::delete(&delete_url).send().await {
                        Ok(response) => {
                            if response.ok() {
                                web_sys::console::log_1(&JsValue::from_str("Image deleted successfully"));
                            } else {
                                web_sys::console::log_1(&JsValue::from_str("Error: Failed to delete the image from the server."));
                            }
                        }
                        Err(err) => {
                            web_sys::console::log_1(&JsValue::from_str(&format!("Error sending delete request: {:?}", err)));
                        }
                    }
                });
            }
        }

        let video_element = video_ref.cast::<HtmlVideoElement>().unwrap();
        let canvas_element = canvas_ref.cast::<HtmlCanvasElement>().unwrap();
        let canvas_context = canvas_element
            .get_context("2d")
            .unwrap()
            .unwrap()
            .dyn_into::<web_sys::CanvasRenderingContext2d>()
            .unwrap();

        canvas_element.set_width(video_element.video_width());
        canvas_element.set_height(video_element.video_height());
        canvas_context.draw_image_with_html_video_element(&video_element, 0.0, 0.0).unwrap();

        let processed_image_url_cloned = processed_image_url.clone();
        let detections_cloned = detections.clone(); // Clone again to avoid capturing in the inner closure

        let closure = Closure::wrap(Box::new(move |blob: web_sys::Blob| {
            let processed_image_url_cloned = processed_image_url_cloned.clone();
            let detections_cloned = detections_cloned.clone(); // Clone `detections_cloned` for the async block

            let form_data = FormData::new().unwrap();
            form_data.append_with_blob("file", &blob).unwrap();

            spawn_local(async move {
                let response = Request::post("http://localhost:8000/process-image")
                    .header("Accept", "application/json")
                    .body(form_data)
                    .unwrap()
                    .send()
                    .await;

                match response {
                    Ok(response) => {
                        if response.ok() {
                            match response.json::<DetectionResponse>().await {
                                Ok(api_response) => {

                                     // Debugging: Print the URL returned by the backend
                                     for detection in &api_response.detections {
                                        web_sys::console::log_1(&JsValue::from_str(&format!(
                                            "Detection - Label: {}",
                                            detection.label
                                        )));
                                    }
                                    web_sys::console::log_1(&JsValue::from_str(&format!("Detections: {}", api_response.detections.len())));

                                    processed_image_url_cloned.set(Some(api_response.url.clone()));
                                    detections_cloned.set(api_response.detections);
                                }
                                Err(err) => {
                                    web_sys::console::log_1(&JsValue::from_str(&format!("Error parsing JSON: {:?}", err)));
                                }
                            }
                        } else {
                            web_sys::console::log_1(&JsValue::from_str("Error: Backend failed to process the image."));
                        }
                    }
                    Err(err) => {
                        web_sys::console::log_1(&JsValue::from_str(&format!("Error sending request: {:?}", err)));
                    }
                }
            });
        }) as Box<dyn FnMut(web_sys::Blob)>);

        
        canvas_element.to_blob_with_type(closure.as_ref().unchecked_ref(), "image/jpeg").unwrap();
        closure.forget();
    })
};


    let add_dominos = {
        let detections = detections.clone();        
        let total_count = total_count.clone();      
        let show_count = show_count.clone();       
    
        Callback::from(move |_| {
            let mut count = 0.0;
            for detection in detections.iter() {
                count += match detection.label.as_str() {
                    "pip-0" => 0.0,
                    "pip-1" => 1.0,
                    "pip-2" => 2.0,
                    "pip-3" => 3.0,
                    "pip-4" => 4.0,
                    "pip-5" => 5.0,
                    "pip-6" => 6.0,
                    "pip-7" => 7.0,
                    "pip-8" => 8.0,
                    "pip-9" => 9.0,
                    "pip-10" => 10.0,
                    "pip-11" => 11.0,
                    "pip-12" => 12.0,
                    _ => 0.0, // Catch-all for any unexpected labels
                };
            }
            total_count.set(count);
            show_count.set(true);
        })
    };


    let best_combo_function = {
        let combo_result = combo_result.clone();      
    
        Callback::from(move |_| {
            // Retrieve the input value
            let document = web_sys::window().unwrap().document().unwrap();
            if let Some(input_element) = document.get_element_by_id("combo_input") {
                let input_value = input_element
                    .dyn_into::<HtmlInputElement>()
                    .unwrap()
                    .value();

                // Parse and calculate the best combo
                let best_combo = calculate_best_combo(&input_value);
                
                // Set the result in state
                combo_result.set(Some(best_combo));
            }
        })
    };







    html! {
        <div class="flex flex-col items-center h-screen py-[10vh] px-6 space-y-6" style="background-color: rgb(198, 237, 245);">
    
            <div class="flex justify-center w-full h-[45vh]">
                <video ref={video_ref} class="border-4 rounded" style="width: 90vw; height: 100%; object-fit: cover; object-position: center; border-color: black;"></video>
            </div>
    
            <div class="flex flex-col items-center space-y-6 w-full px-6">
                <div class="flex flex-row items-center space-x-4">
                    <button class="px-10 py-6 bg-green-500 text-white text-4xl font-bold rounded-lg shadow-md hover:bg-green-700" onclick={request_camera}>
                        { "Access Camera" }
                    </button>
                    <button class="px-10 py-6 bg-blue-500 text-white text-4xl font-bold rounded-lg shadow-md hover:bg-blue-700" onclick={capture_image}>
                        { "Capture Image" }
                    </button>
    
                    if let Some(url) = &*processed_image_url {
                        <button class="px-10 py-6 bg-blue-500 text-white text-4xl font-bold rounded-lg shadow-md hover:bg-blue-700" onclick={add_dominos}>
                            { "Add Dominos" }
                        </button>
                    }
                </div>
    
                if *show_count {
                    <p class="text-4xl font-bold">{ format!("Total Count = {}", *total_count) }</p>
                }
                <p class="text-2xl font-bold"> { "Enter In Form: START# NUM1,NUM2 SPACE NUM1,NUM2... ex: 1 3,1 12,0" } </p>
                // New section with user input and "Best Combo" button
                <div class="flex flex-row items-center space-x-4 mt-4 w-full px-6">
                    
                    <input type="text" class="text-2xl p-4 border border-gray-300 rounded-lg w-full" placeholder="Enter value here..." />
                    <button class="px-8 py-4 bg-purple-500 text-white text-2xl font-bold rounded-lg shadow-md hover:bg-purple-700" onclick={best_combo_function}>
                        { "Best Combo" }
                    </button>
                </div>
            </div>
    
            if let Some(url) = &*processed_image_url {
                <div class="flex justify-center w-full h-[45vh]">
                    <img class="border-4 border-gray-300 rounded" style="width: 90vw; height: 100%; object-fit: cover; object-position: center;" src={url.clone()} alt="Processed Image" />
                </div>
            }
    
            <canvas ref={canvas_ref} class="hidden"></canvas>
        </div>
    }
    

}

#[wasm_bindgen(start)]
pub fn run_app() {
    yew::Renderer::<App>::new().render();
}

fn main() {}