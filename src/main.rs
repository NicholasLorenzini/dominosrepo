use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{HtmlCanvasElement, HtmlVideoElement, MediaStream, FormData};
use yew::prelude::*;
use gloo_net::http::Request;
use wasm_bindgen_futures::spawn_local;
use js_sys::Reflect;
use serde_json::json;
use gloo_utils::format::JsValueSerdeExt;

#[function_component(App)]
fn app() -> Html {
    let video_ref = use_node_ref();
    let canvas_ref = use_node_ref();
    let processed_image_url = use_state(|| None);

    let request_camera = {
        let video_ref = video_ref.clone();
        Callback::from(move |_| {
            let video_ref = video_ref.clone();
            let window = web_sys::window().unwrap();
            let navigator = window.navigator();
            let media_devices = navigator.media_devices().unwrap();

            // Request camera access
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
        Callback::from(move |_| {
            let video_element = video_ref.cast::<HtmlVideoElement>().unwrap();
            let canvas_element = canvas_ref.cast::<HtmlCanvasElement>().unwrap();
            let canvas_context = canvas_element
                .get_context("2d")
                .unwrap()
                .unwrap()
                .dyn_into::<web_sys::CanvasRenderingContext2d>()
                .unwrap();

            // Draw the current video frame to the canvas
            canvas_element.set_width(video_element.video_width());
            canvas_element.set_height(video_element.video_height());

            canvas_context.draw_image_with_html_video_element(&video_element, 0.0, 0.0).unwrap();

            // Clone `processed_image_url` to avoid moving it into the closure
            let processed_image_url = processed_image_url.clone();

            // Create a closure to handle the blob asynchronously
            let closure = Closure::wrap(Box::new(move |blob: web_sys::Blob| {
                let form_data = FormData::new().unwrap();
                form_data.append_with_blob("file", &blob).unwrap();

                let processed_image_url = processed_image_url.clone(); // Clone again here to avoid move

                spawn_local(async move {
                    let response = Request::post("http://localhost:8000/process-image")
                        .header("Accept", "image/jpeg")  // Expect a JPG image
                        .body(form_data)
                        .unwrap()
                        .send()
                        .await;

                    match response {
                        Ok(response) => {
                            if response.ok() {
                                // Get the binary data and convert to a blob
                                let processed_image_data = response.binary().await.unwrap();
                                let array = js_sys::Uint8Array::from(processed_image_data.as_slice());
                                let processed_blob = web_sys::Blob::new_with_u8_array_sequence(&array.into()).unwrap();
                                
                                // Create a blob URL for the image
                                let blob_url = web_sys::Url::create_object_url_with_blob(&processed_blob).unwrap();
                                
                                // Set the processed image URL
                                processed_image_url.set(Some(blob_url));
                            } else {
                                web_sys::console::log_1(&JsValue::from_str("Error: Backend failed to process the image."));
                            }
                        },
                        Err(err) => {
                            web_sys::console::log_1(&JsValue::from(format!("Error sending request: {:?}", err)));
                        }
                    }
                });
            }) as Box<dyn FnMut(web_sys::Blob)>);

            // Call `to_blob` asynchronously, passing the closure as the callback
            canvas_element.to_blob(closure.as_ref().unchecked_ref()).unwrap();
            closure.forget(); // Prevent memory leaks by forgetting the closure
        })
    };

    html! {
        <div class="flex flex-col items-center justify-center h-screen space-y-6">
            <video ref={video_ref} class="border-4 border-blue-500 rounded" width="640" height="480"></video>

            <div class="space-x-4">
                <button class="px-4 py-2 bg-green-500 text-white rounded-lg shadow-md hover:bg-green-700" onclick={request_camera}>{ "Access Camera" }</button>
                <button class="px-4 py-2 bg-blue-500 text-white rounded-lg shadow-md hover:bg-blue-700" onclick={capture_image}>{ "Capture Image" }</button>
            </div>

            <canvas ref={canvas_ref} class="hidden"></canvas>

            // Display the processed image if the URL is available
            if let Some(url) = &*processed_image_url {
                <img class="mt-4 border-4 border-gray-300 rounded-lg" src={url.clone()} alt="Processed Image" />
            }
        </div>
    }
}

#[wasm_bindgen(start)]
pub fn run_app() {
    yew::Renderer::<App>::new().render();
}

fn main() {}
