use cfg_if::cfg_if;
use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use wasm_bindgen::{prelude::*, JsCast};
use web_sys::{js_sys, CanvasRenderingContext2d};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DrawEvent {
    pub event_type: String,
    pub x: f64,
    pub y: f64,
    pub prev_x: Option<f64>,
    pub prev_y: Option<f64>,
    pub color: String,
    pub brush_size: u32,
    pub room_id: String,
    pub user_id: String,
}

#[component]
pub fn DrawingCanvas() -> impl IntoView {
    let (connected, set_connected) = signal(false);
    let (color, set_color) = signal(String::from("#000000"));
    let (brush_size, set_brush_size) = signal(5);
    let (is_drawing, set_is_drawing) = signal(false);
    let (last_x, set_last_x) = signal(0.0);
    let (last_y, set_last_y) = signal(0.0);
    let (room_id, _set_room_id) = signal(String::from("default-room"));
    let (user_id, _set_user_id) = signal(generate_user_id());

    // Canvas reference
    let canvas_ref = NodeRef::<leptos::html::Canvas>::new();

    // Store WebSocket in a thread-local storage
    thread_local! {
        static WEBSOCKET: RefCell<Option<web_sys::WebSocket>> = const {RefCell::new(None) };
    }

    // Initialize WebSocket on component mount
    let setup_websocket = move || {
        let protocol = if window().location().protocol().unwrap() == "https:" {
            "wss"
        } else {
            "ws"
        };

        let ws_url = format!(
            "{}://{}/ws/drawing",
            protocol,
            window().location().host().unwrap()
        );
        log::info!("Connecting to WebSocket at {ws_url}");

        match web_sys::WebSocket::new(&ws_url) {
            Ok(ws) => {
                let open_closure = Closure::wrap(Box::new(move || {
                    set_connected.set(true);
                    log::info!("WebSocket connected");
                }) as Box<dyn FnMut()>);

                let close_closure = Closure::wrap(Box::new(move || {
                    set_connected.set(false);
                    log::info!("WebSocket disconnected");
                }) as Box<dyn FnMut()>);

                let user_id_clone = user_id.get();
                let message_closure = Closure::wrap(Box::new(move |e: web_sys::MessageEvent| {
                    if let Some(text) = e.data().as_string() {
                        match serde_json::from_str::<DrawEvent>(&text) {
                            Ok(event) => {
                                // Only process events from other users
                                if event.user_id != user_id_clone {
                                    if let Some(canvas) = canvas_ref.get() {
                                        draw_event_on_canvas(&canvas, &event);
                                    }
                                }
                            }
                            Err(e) => {
                                log::info!("Failed to parse event: {e}");
                            }
                        }
                    }
                })
                    as Box<dyn FnMut(web_sys::MessageEvent)>);

                ws.set_onopen(Some(open_closure.as_ref().unchecked_ref()));
                ws.set_onclose(Some(close_closure.as_ref().unchecked_ref()));
                ws.set_onmessage(Some(message_closure.as_ref().unchecked_ref()));

                // Store closures to prevent them from being garbage collected
                open_closure.forget();
                close_closure.forget();
                message_closure.forget();

                // Store WebSocket instance
                WEBSOCKET.with(|ws_cell| {
                    *ws_cell.borrow_mut() = Some(ws);
                });
            }
            Err(err) => {
                log::info!("Failed to connect to WebSocket: {err:?}");
            }
        }
    };

    // Initialize WebSocket on component mount
    let _ = RenderEffect::new(move |_| {
        setup_websocket();

        // Cleanup function
        || {
            WEBSOCKET.with(|ws_cell| {
                if let Some(ws) = ws_cell.borrow_mut().take() {
                    ws.close().unwrap_or_default();
                }
            });
        }
    });

    // Function to send an event to the WebSocket
    let send_event = move |event: DrawEvent| {
        WEBSOCKET.with(|ws_cell| {
            if let Some(ws) = ws_cell.borrow().as_ref() {
                if let Ok(json) = serde_json::to_string(&event) {
                    let _ = ws.send_with_str(&json);
                }
            }
        });
    };

    // Function to draw on the canvas
    let draw_line = move |x1: f64, y1: f64, x2: f64, y2: f64| {
        if let Some(canvas) = canvas_ref.get() {
            let context = canvas
                .get_context("2d")
                .unwrap()
                .unwrap()
                .dyn_into::<CanvasRenderingContext2d>()
                .unwrap();

            context.begin_path();
            context.move_to(x1, y1);
            context.line_to(x2, y2);
            context.set_line_cap("round");
            context.set_line_width(brush_size.get() as f64);
            context.set_fill_style_str(&color.get());
            context.set_stroke_style_str(&color.get());
            context.stroke();

            // Send drawing event to server
            let event = DrawEvent {
                event_type: "line".to_string(),
                x: x2,
                y: y2,
                prev_x: Some(x1),
                prev_y: Some(y1),
                color: color.get(),
                brush_size: brush_size.get(),
                room_id: room_id.get(),
                user_id: user_id.get(),
            };

            send_event(event);
        }
    };

    // Canvas mouse event handlers
    let on_mouse_down = move |e: web_sys::MouseEvent| {
        set_is_drawing.set(true);
        set_last_x.set(e.offset_x() as f64);
        set_last_y.set(e.offset_y() as f64);
    };

    let on_mouse_move = move |e: web_sys::MouseEvent| {
        if is_drawing.get() {
            let x = e.offset_x() as f64;
            let y = e.offset_y() as f64;
            draw_line(last_x.get(), last_y.get(), x, y);
            set_last_x.set(x);
            set_last_y.set(y);
        }
    };

    let on_mouse_up = move |_| {
        set_is_drawing.set(false);
    };

    let on_mouse_out = move |_| {
        set_is_drawing.set(false);
    };

    let on_touch_start = move |e: web_sys::TouchEvent| {
        e.prevent_default();
        let touches = e.touches();
        if touches.length() > 0 {
            if let Some(touch) = js_sys::try_iter(&touches)
                .unwrap()
                .unwrap()
                .next()
                .and_then(Result::ok)
            {
                let touch: web_sys::Touch = touch.dyn_into().unwrap();
                set_is_drawing.set(true);

                if let Some(canvas) = canvas_ref.get() {
                    let rect = canvas.get_bounding_client_rect();
                    let x = touch.client_x() as f64 - rect.left();
                    let y = touch.client_y() as f64 - rect.top();

                    set_last_x.set(x);
                    set_last_y.set(y);
                }
            }
        }
    };

    let on_touch_move = move |e: web_sys::TouchEvent| {
        e.prevent_default();
        if is_drawing.get() {
            let touches = e.touches();
            if touches.length() > 0 {
                if let Some(touch) = js_sys::try_iter(&touches)
                    .unwrap()
                    .unwrap()
                    .next()
                    .and_then(Result::ok)
                {
                    let touch: web_sys::Touch = touch.dyn_into().unwrap();

                    if let Some(canvas) = canvas_ref.get() {
                        let rect = canvas.get_bounding_client_rect();
                        let x = touch.client_x() as f64 - rect.left();
                        let y = touch.client_y() as f64 - rect.top();

                        draw_line(last_x.get(), last_y.get(), x, y);
                        set_last_x.set(x);
                        set_last_y.set(y);
                    }
                }
            }
        }
    };

    let on_touch_end = move |e: web_sys::TouchEvent| {
        e.prevent_default();
        set_is_drawing.set(false);
    };

    let on_touch_cancel = move |e: web_sys::TouchEvent| {
        e.prevent_default();
        set_is_drawing.set(false);
    };

    let clear_canvas = move |_| {
        if let Some(canvas) = canvas_ref.get() {
            let context = canvas
                .get_context("2d")
                .unwrap()
                .unwrap()
                .dyn_into::<CanvasRenderingContext2d>()
                .unwrap();

            context.clear_rect(0.0, 0.0, canvas.width() as f64, canvas.height() as f64);

            // Send clear event to server
            let event = DrawEvent {
                event_type: "clear".to_string(),
                x: 0.0,
                y: 0.0,
                prev_x: None,
                prev_y: None,
                color: color.get(),
                brush_size: brush_size.get(),
                room_id: room_id.get(),
                user_id: user_id.get(),
            };

            send_event(event);
        }
    };

    view! {
        <div class="flex flex-col items-center">
            <h2 class="text-2xl font-bold mb-4">"Collaborative Drawing"</h2>

            <div class="mb-4 flex items-center space-x-4">
                <div class="flex items-center">
                    <label for="color-picker" class="mr-2">
                        "Color:"
                    </label>
                    <input
                        id="color-picker"
                        type="color"
                        value=color
                        on:change=move |e| {
                            set_color.set(event_target_value(&e));
                        }

                        class="h-8 w-12"
                    />
                </div>

                <div class="flex items-center">
                    <label for="brush-size" class="mr-2">
                        "Brush Size:"
                    </label>
                    <input
                        id="brush-size"
                        type="range"
                        min="1"
                        max="30"
                        value=brush_size
                        on:input=move |e| {
                            set_brush_size.set(event_target_value(&e).parse::<u32>().unwrap_or(5));
                        }

                        class="w-32"
                    />
                    <span class="ml-2">{brush_size}</span>
                </div>

                <button
                    on:click=clear_canvas
                    class="bg-red-500 hover:bg-red-600 text-white px-4 py-2 rounded"
                >
                    "Clear Canvas"
                </button>
            </div>

            <div class="border-2 border-gray-300 rounded">
                <canvas
                    node_ref=canvas_ref
                    width="800"
                    height="600"
                    on:mousedown=on_mouse_down
                    on:mousemove=on_mouse_move
                    on:mouseup=on_mouse_up
                    on:mouseout=on_mouse_out
                    on:touchstart=on_touch_start
                    on:touchmove=on_touch_move
                    on:touchend=on_touch_end
                    on:touchcancel=on_touch_cancel
                    class="bg-white touch-auto"
                ></canvas>
            </div>

            <div class="mt-4 text-sm text-gray-500">
                {move || {
                    if connected.get() {
                        "Connected to drawing room"
                    } else {
                        "Disconnected from drawing room"
                    }
                }}

            </div>
        </div>
    }
}

// Helper function to generate a random user ID
fn generate_user_id() -> String {
    cfg_if! {
        if #[cfg(feature = "hydrate")] {
            use web_sys::js_sys::Math;
            format!("user-{}", (Math::random() * 10000.0) as u32)
        } else {
            return "server-side-user".to_string();
        }
    }
}

// Function to draw an event received from another user
fn draw_event_on_canvas(_canvas: &web_sys::HtmlCanvasElement, _event: &DrawEvent) {
    cfg_if! {
        if #[cfg(feature = "hydrate")] {
            let context = _canvas
                .get_context("2d")
                .unwrap()
                .unwrap()
                .dyn_into::<CanvasRenderingContext2d>()
                .unwrap();

            match _event.event_type.as_str() {
                "line" => {
                    if let (Some(prev_x), Some(prev_y)) = (_event.prev_x, _event.prev_y) {
                        context.begin_path();
                        context.move_to(prev_x, prev_y);
                        context.line_to(_event.x, _event.y);
                        context.set_line_cap("round");
                        context.set_line_width(_event.brush_size as f64);
                        context.set_fill_style_str(&_event.color);
                        context.set_stroke_style_str(&_event.color);
                        context.stroke();
                    }
                }
                "clear" => {
                    context.clear_rect(0.0, 0.0, _canvas.width() as f64, _canvas.height() as f64);
                }
                _ => {}
            }
        } else {
            // No-op for server-side rendering
        }
    }
}
