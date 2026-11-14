use gloo_timers::callback::Interval;
use serde_json::{json, Value};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::{spawn_local, JsFuture};
use web_sys::{HtmlInputElement, HtmlSelectElement, HtmlTextAreaElement};
use yew::prelude::*;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "core"], js_name = invoke)]
    fn invoke_js(command: &str, args: JsValue) -> js_sys::Promise;
}
async fn invoke(command: &str, args: Value) -> Result<Value, String> {
    let args = serde_wasm_bindgen::to_value(&args).map_err(|e| e.to_string())?;
    let result = JsFuture::from(invoke_js(command, args))
        .await
        .map_err(|e| e.as_string().unwrap_or_else(|| format!("{e:?}")))?;
    serde_wasm_bindgen::from_value(result).map_err(|e| e.to_string())
}
fn dispatch(request: Value, error: UseStateHandle<String>) {
    spawn_local(async move {
        error.set(String::new());
        if let Err(message) = invoke("action", json!({"request": request})).await {
            error.set(message);
        }
    });
}
fn copy(text: String, error: UseStateHandle<String>) -> Callback<MouseEvent> {
    Callback::from(move |_| {
        let text = text.clone();
        let error = error.clone();
        spawn_local(async move {
            if let Err(message) = invoke("copy_text", json!({"text": text})).await {
                error.set(message);
            }
        });
    })
}
fn s<'a>(v: &'a Value, field: &str) -> &'a str {
    v[field].as_str().unwrap_or("")
}

#[derive(Properties, PartialEq)]
struct SettingsProps {
    settings: Value,
    error: UseStateHandle<String>,
}
#[function_component(Settings)]
fn settings(props: &SettingsProps) -> Html {
    let draft = use_state(|| props.settings.clone());
    let update = |field: &'static str| {
        let draft = draft.clone();
        Callback::from(move |event: InputEvent| {
            let input: HtmlInputElement = event.target_unchecked_into();
            let mut next = (*draft).clone();
            next[field] = json!(input.value());
            draft.set(next);
        })
    };
    let change_text = |field: &'static str| {
        let draft = draft.clone();
        Callback::from(move |event: InputEvent| {
            let input: HtmlTextAreaElement = event.target_unchecked_into();
            let mut next = (*draft).clone();
            next[field] = json!(input.value());
            draft.set(next);
        })
    };
    let mode = {
        let draft = draft.clone();
        Callback::from(move |event: Event| {
            let input: HtmlSelectElement = event.target_unchecked_into();
            let mut next = (*draft).clone();
            next["mode"] = json!(input.value());
            draft.set(next);
        })
    };
    let layers = {
        let draft = draft.clone();
        Callback::from(move |event: InputEvent| {
            let input: HtmlInputElement = event.target_unchecked_into();
            if let Ok(value) = input.value().parse::<u32>() {
                let mut next = (*draft).clone();
                next["gpu_layers"] = json!(value);
                draft.set(next);
            }
        })
    };
    let toggle = |field: &'static str| {
        let draft = draft.clone();
        Callback::from(move |event: Event| {
            let input: HtmlInputElement = event.target_unchecked_into();
            let mut next = (*draft).clone();
            next[field] = json!(input.checked());
            draft.set(next);
        })
    };
    let save = {
        let draft = draft.clone();
        let error = props.error.clone();
        Callback::from(move |_| {
            dispatch(json!({"action":"save", "settings": *draft}), error.clone());
        })
    };
    html! { <section class="settings">
        <h2>{"Make it yours"}</h2>
        <p class="muted">{"Models stay on your machine. Changes apply to the next dictation."}</p>
        <label>{"Cleanup level"}<select onchange={mode} value={s(&draft,"mode").to_owned()}>
            {for ["raw","light","standard","aggressive","custom"].iter().map(|m| html!{<option value={m.to_string()} selected={s(&draft,"mode")==*m}>{m}</option>})}
        </select></label>
        <p class="hint">{"Raw keeps the transcript. Light fixes grammar. Standard resolves corrections. Aggressive also rewrites and structures your final intention."}</p>
        <label>{"Custom cleanup instructions"}<textarea rows="3" value={s(&draft,"custom").to_owned()} oninput={change_text("custom")}/></label>
        <label>{"Vocabulary"}<textarea rows="3" value={s(&draft,"vocabulary").to_owned()} oninput={change_text("vocabulary")}/></label>
        <p class="hint">{"Names, filenames, and identifiers to help cleanup preserve the right spelling. These hints do not retrain speech recognition."}</p>
        <h3>{"Local models"}</h3>
        <label>{"Cohere Q8_0 GGUF"}<input value={s(&draft,"asr_model").to_owned()} oninput={update("asr_model")}/></label>
        <label>{"CrispASR executable"}<input value={s(&draft,"crispasr").to_owned()} oninput={update("crispasr")}/></label>
        <label>{"Qwen Q4_K_M GGUF"}<input value={s(&draft,"gguf").to_owned()} oninput={update("gguf")}/></label>
        <label>{"CUDA llama-server executable"}<input value={s(&draft,"llama_server").to_owned()} oninput={update("llama_server")}/></label>
        <label>{"Qwen layers on GPU (0 to 36)"}<input type="number" min="0" max="36" value={draft["gpu_layers"].to_string()} oninput={layers}/></label>
        <p class="hint">{"24 is a conservative starting point for 8 GB GPUs. The remaining layers run on CPU. Check logs for CUDA allocation and lower this if memory runs out."}</p>
        <label class="check"><input type="checkbox" checked={draft["auto_paste"].as_bool().unwrap_or(true)} onchange={toggle("auto_paste")}/>{"Automatically paste into the original application"}</label>
        <label class="check"><input type="checkbox" checked={draft["keep_history"].as_bool().unwrap_or(true)} onchange={toggle("keep_history")}/>{"Save dictation history locally (up to 500 entries)"}</label>
        <button class="primary" onclick={save}>{"Save settings"}</button>
        <h3>{"Push to talk on Hyprland"}</h3>
        <p>{"Start rafiFlow, then add the press and release bindings from the README. The default example uses Ctrl + Alt + Space. Keep the receiving app focused while speaking."}</p>
        <p class="hint">{"Your existing desktop configuration is never changed automatically."}</p>
    </section> }
}

#[function_component(App)]
fn app() -> Html {
    let snapshot = use_state(|| json!({}));
    let error = use_state(String::new);
    let dark = use_state(|| {
        web_sys::window()
            .and_then(|window| window.local_storage().ok().flatten())
            .and_then(|storage| storage.get_item("rafiflow.appearance").ok().flatten())
            .as_deref()
            != Some("light")
    });
    let page = use_state(|| "history".to_string());
    let search = use_state(String::new);
    let test_text = use_state(String::new);
    {
        let snapshot = snapshot.clone();
        let error = error.clone();
        use_effect_with((), move |_| {
            let tick = move || {
                let snapshot = snapshot.clone();
                let error = error.clone();
                spawn_local(async move {
                    match invoke("snapshot", json!({})).await {
                        Ok(value) => snapshot.set(value),
                        Err(message) => error.set(message),
                    }
                });
            };
            tick();
            let interval = Interval::new(500, tick);
            move || drop(interval)
        });
    }
    let status = &snapshot["status"];
    let phase = s(status, "phase");
    let busy = matches!(
        phase,
        "starting" | "recording" | "transcribing" | "cleaning"
    );
    let overlay = web_sys::window()
        .unwrap()
        .location()
        .search()
        .unwrap_or_default()
        .contains("overlay");
    if overlay {
        if phase != "recording" {
            return html! {<div class="overlay-page"/>};
        }
        return html! {<div class="overlay-page"><div class="overlay listening">
            <span class="record-dot"/><div class="wave" aria-hidden="true">
            {for (0..15).map(|i| html!{<i style={format!("--i:{i};--level:{}", status["level"].as_f64().unwrap_or(0.0))}/>})}
            </div><span>{s(status,"message")}</span>
        </div></div>};
    }
    let toggle_theme = {
        let dark = dark.clone();
        let error = error.clone();
        Callback::from(move |_| {
            let next = !*dark;
            dark.set(next);
            let saved = web_sys::window()
                .and_then(|window| window.local_storage().ok().flatten())
                .is_some_and(|storage| {
                    storage
                        .set_item("rafiflow.appearance", if next { "dark" } else { "light" })
                        .is_ok()
                });
            if !saved {
                error.set("Appearance changed, but it couldn't be saved for next time.".into());
            }
        })
    };
    let nav = |name: &'static str| {
        let page = page.clone();
        Callback::from(move |_| page.set(name.into()))
    };
    let start = {
        let error = error.clone();
        Callback::from(move |_| dispatch(json!({"action":"start"}), error.clone()))
    };
    let stop = {
        let error = error.clone();
        Callback::from(move |_| dispatch(json!({"action":"stop"}), error.clone()))
    };
    let cancel = {
        let error = error.clone();
        Callback::from(move |_| dispatch(json!({"action":"cancel"}), error.clone()))
    };
    let run = {
        let error = error.clone();
        let test_text = test_text.clone();
        Callback::from(move |_| dispatch(json!({"action":"test","text":*test_text}), error.clone()))
    };
    let search_input = {
        let search = search.clone();
        Callback::from(move |event: InputEvent| {
            search.set(event.target_unchecked_into::<HtmlInputElement>().value())
        })
    };
    let test_input = {
        let test_text = test_text.clone();
        Callback::from(move |event: InputEvent| {
            test_text.set(event.target_unchecked_into::<HtmlTextAreaElement>().value())
        })
    };
    let clear = {
        let error = error.clone();
        Callback::from(move |_| {
            if web_sys::window()
                .unwrap()
                .confirm_with_message("Delete all saved dictation history?")
                .unwrap_or(false)
            {
                dispatch(json!({"action":"clear_history"}), error.clone());
            }
        })
    };
    html! { <div class={classes!("app", (!*dark).then_some("light"))}>
        <aside>
            <div class="brand"><span class="brand-mark">{"〰"}</span><strong>{"rafiFlow"}</strong></div>
            <p class="tagline">{"Think aloud. Write clearly."}</p>
            <nav>
                <button class={classes!((*page=="history").then_some("selected"))} onclick={nav("history")}>{"Dictation history"}</button>
                <button class={classes!((*page=="cleanup").then_some("selected"))} onclick={nav("cleanup")}>{"Cleanup studio"}</button>
                <button class={classes!((*page=="settings").then_some("selected"))} onclick={nav("settings")}>{"Settings & setup"}</button>
            </nav>
            <div class="privacy"><span class="status-dot"/>{"Fully local"}<p>{"No account. No uploads."}</p></div>
        </aside>
        <main>
            <header><div><h1>{match page.as_str() {"settings"=>"Your flow", "cleanup"=>"From thought to text", _=>"Your words, collected"}}</h1><p class="muted">{"English dictation, entirely on your machine."}</p></div>
            <button class="theme-toggle" type="button" onclick={toggle_theme}
                aria-label={if *dark {"Switch to light mode"} else {"Switch to dark mode"}}>
                {if *dark {"Light mode"} else {"Dark mode"}}
            </button></header>
            <div class={classes!("status", (phase=="error").then_some("error"))} role="status">
                <span class={classes!("status-dot", (phase=="recording").then_some("pulse"))}/>
                <div><strong>{if matches!(phase,"transcribing" | "cleaning") {"Recording finished"} else {s(status,"message")}}</strong><small>{if status["models_loaded"]==true {"Models warm · unload after five idle minutes"} else {"Models load on first use"}}</small></div>
                {if phase=="recording" {html!{<><button onclick={stop}>{"Finish"}</button><button onclick={cancel}>{"Cancel"}</button></>}} else {html!{<button disabled={busy} onclick={start}>{"Record"}</button>}}}
            </div>
            {if !error.is_empty() {html!{<p class="error notice" role="alert">{(*error).clone()}</p>}} else {Html::default()}}
            {match page.as_str() {
                "settings" => if snapshot["settings"].is_object() {html!{<Settings settings={snapshot["settings"].clone()} error={error.clone()}/>}} else {html!{<p>{"Loading settings…"}</p>}},
                "cleanup" => html!{<section>
                    <h2>{"Try a thought. Compare the result."}</h2>
                    <p class="muted">{"Paste a raw transcript to test your selected cleanup level without recording or pasting into another application."}</p>
                    <p class="mode-label">{format!("Current level: {}", s(&snapshot["settings"],"mode"))}<button onclick={nav("settings")}>{"Change level"}</button></p>
                    <textarea class="test-input" rows="8" placeholder="I want to update the project AGENTS.md, actually the global one. Scratch that, just give me a plan…" value={(*test_text).clone()} oninput={test_input}/>
                    <button class="primary" disabled={busy || test_text.trim().is_empty()} onclick={run}>{"Clean up text"}</button>
                    {if status["last"].is_object() {html!{<article><h3>{"Latest result"}</h3><pre>{s(&status["last"],"text")}</pre><button onclick={copy(s(&status["last"],"text").into(),error.clone())}>{"Copy result"}</button></article>}} else {Html::default()}}
                </section>},
                _ => {
                    let entries = snapshot["history"].as_array().cloned().unwrap_or_default();
                    let query = search.to_lowercase();
                    html!{<section>
                        <div class="toolbar"><input aria-label="Search history" placeholder="Search your dictations" value={(*search).clone()} oninput={search_input}/><button disabled={busy} onclick={clear}>{"Delete history"}</button></div>
                        {if entries.is_empty() {html!{<div class="empty"><div class="empty-wave">{"〰"}</div><h2>{"A little less typing."}</h2><p>{"Set up your models and shortcut, focus any text field, then hold to speak."}</p><button class="primary" onclick={nav("settings")}>{"Set up rafiFlow"}</button></div>}} else {html!{<div class="entries">{for entries.iter().filter(|e|format!("{} {}",s(e,"raw"),s(e,"text")).to_lowercase().contains(&query)).map(|entry| html!{
                            <article key={s(entry,"id").to_owned()}>
                                <div class="entry-heading"><small>{format!("{} · {}",s(entry,"mode"),js_sys::Date::new(&JsValue::from_f64(entry["timestamp"].as_f64().unwrap_or(0.0)*1000.0)).to_utc_string())}</small><button onclick={copy(s(entry,"text").into(),error.clone())}>{"Copy"}</button></div>
                                <pre>{s(entry,"text")}</pre>
                                {if let Some(message)=entry["error"].as_str() {html!{<p class="error">{message}</p>}} else {Html::default()}}
                                <details><summary>{"Original transcript"}</summary><pre class="raw">{s(entry,"raw")}</pre></details>
                            </article>
                        })}</div>}}}
                    </section>}
                }
            }}
        </main>
    </div>}
}
fn main() {
    yew::Renderer::<App>::new().render();
}
