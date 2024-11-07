use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{HtmlCanvasElement, HtmlVideoElement, HtmlInputElement, MediaStream, FormData};
use yew::prelude::*;
use gloo_net::http::Request;
use wasm_bindgen_futures::spawn_local;
use js_sys::Reflect;
use serde_json::json;
use gloo_utils::format::JsValueSerdeExt;
use serde::Deserialize;
use regex::Regex;



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
    
                // Debug: Log the retrieved input
                web_sys::console::log_1(&JsValue::from_str(&format!("Input value: {}", input_value)));
    
                // Parse and calculate the best combo
                let best_combo = calculate_best_combo(&input_value);
    
                // Debug: Log the result of calculate_best_combo
    
                // Set the result in state
                combo_result.set(best_combo);
            }
        })
    };
    
    fn calculate_best_combo(input: &str) -> Option<String> {
        // Define the regex pattern for the input format
        let re = Regex::new(r"^\d+\s(\d+,\d+\s?)+$").unwrap();
    
        // Check if the input matches the expected format
        if !re.is_match(input) {
            web_sys::console::log_1(&JsValue::from_str("Input format is incorrect"));
            return Some("Input format is incorrect".to_string());
        }
    
        // Split the input string by whitespace to separate START# from the pairs
        let mut parts = input.split_whitespace();
        let start = parts.next().unwrap().parse::<i32>().ok()?;
        web_sys::console::log_1(&JsValue::from_str(&format!("Start number: {}", start)));
    
        // Parse each NUM1,NUM2 pair as a tuple
        let mut pairs: Vec<(i32, i32)> = Vec::new();
        for pair_str in parts {
            let nums: Vec<&str> = pair_str.split(',').collect();
            if nums.len() == 2 {
                let num1 = nums[0].parse::<i32>().ok()?;
                let num2 = nums[1].parse::<i32>().ok()?;
                pairs.push((num1, num2));
            }
        }
    
        // Debug: Log the parsed pairs
        web_sys::console::log_1(&JsValue::from_str(&format!("Parsed pairs: {:?}", pairs)));
    
        // Find the longest chain starting from the initial start number
        let longest_chain = find_longest_chain(start, &pairs, Vec::new());
    
        // Debug: Log the longest chain found
        web_sys::console::log_1(&JsValue::from_str(&format!("Longest chain: {:?}", longest_chain)));
    
        // Format the chain as a string
        let mut formatted_chain = String::from("[");
        for (i, &(num1, num2)) in longest_chain.iter().enumerate() {
            if i > 0 {
                formatted_chain.push_str(", ");
            }
            formatted_chain.push_str(&format!("[{}, {}]", num1, num2));
        }
        formatted_chain.push(']');
    
        Some(formatted_chain)
    }
    
    fn find_longest_chain(start: i32, pairs: &[(i32, i32)], current_chain: Vec<(i32, i32)>) -> Vec<(i32, i32)> {
        let mut longest_chain = current_chain.clone();
    
        for (index, &next_pair) in pairs.iter().enumerate() {
            if next_pair.0 == start || next_pair.1 == start {
                // Calculate the next starting point
                let next_start = if next_pair.0 == start { next_pair.1 } else { next_pair.0 };
    
                // Ensure the pair is in the format [start, next_start]
                let ordered_pair = if next_pair.0 == start {
                    (next_pair.0, next_pair.1)
                } else {
                    (next_pair.1, next_pair.0)
                };
    
                // Remove the current pair and recursively find the next chain
                let mut remaining_pairs = pairs.to_vec();
                remaining_pairs.remove(index);
    
                // Recurse with the updated starting point and chain
                let mut new_chain = current_chain.clone();
                new_chain.push(ordered_pair);
                let candidate_chain = find_longest_chain(next_start, &remaining_pairs, new_chain);
    
                // Update the longest chain if the new candidate is longer
                if candidate_chain.len() > longest_chain.len() {
                    longest_chain = candidate_chain;
                }
            }
        }
    
        // Debug: Log the current longest chain at this recursion level
        web_sys::console::log_1(&JsValue::from_str(&format!("Current longest chain for start {}: {:?}", start, longest_chain)));
        //test 1 3,1 4,1 5,1 5,4 3,2 12,1 3,12
        longest_chain
    }




    html! {
        <div class="flex flex-col items-center h-screen py-[10vh] px-6 space-y-6 text-white" style="background-color: rgb(48, 47, 54);">
        
            <div class="flex justify-center w-full h-[45vh]">
                <video ref={video_ref} class="border-4 rounded-lg" style="width: 90vw; height: 100%; object-fit: cover; object-position: center; border-color: black;"></video>
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
                        <button class="px-10 py-6 text-white text-4xl font-bold rounded-lg shadow-md hover:opacity-90" style="background-color: rgb(194, 130, 58);" onclick={add_dominos}>
                            { "Add Dominos" }
                        </button>
                    }
                </div>
    
                if *show_count {
                    <p class="text-4xl font-bold">{ format!("Total Count = {}", *total_count) }</p>
                }
                <p class="text-2xl font-bold"> { "Enter In Form: START# NUM1,NUM2 SPACE NUM1,NUM2... ex: 1 3,1 12,0" } </p>
                
                <div class="flex flex-row items-center space-x-4 mt-4 w-full px-6">
                    
                    <input type="text" id="combo_input" class="text-2xl p-4 border border-gray-300 rounded-lg w-full bg-gray-800 text-white" placeholder="Enter value here..." />
                    <button class="px-8 py-4 bg-purple-500 text-white text-2xl font-bold rounded-lg shadow-md hover:bg-purple-700" onclick={best_combo_function}>
                        { "Best Combo" }
                    </button>
                </div>
                if let Some(combo) = &*combo_result {
                    <p class="text-3xl font-bold">{ format!("Best Combo: {}", combo) }</p>
                }
    
            </div>
    
            if let Some(url) = &*processed_image_url {
                <div class="flex justify-center w-full h-[45vh]">
                    <img class="border-4 rounded-lg" style="width: 90vw; height: 100%; object-fit: cover; object-position: center; border-color: black;" src={url.clone()} alt="Processed Image" />
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