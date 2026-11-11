use crate::{commands, llm, service::Service};
use pluely_core::model::{Error, Result, Settings};
use std::sync::Arc;
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Emitter, Manager, WebviewUrl, WebviewWindowBuilder,
};
use tauri_plugin_autostart::ManagerExt as AutostartExt;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};
fn parse(raw: &str) -> Result<Shortcut> {
    raw.replace("CommandOrControl", "CmdOrCtrl")
        .replace('\\', "Backslash")
        .parse()
        .map_err(|_| Error::Invalid(format!("Invalid shortcut: {raw}")))
}
pub fn register_shortcuts(app: &tauri::AppHandle, settings: &Settings) -> Result<()> {
    let values = [
        &settings.shortcuts.visibility,
        &settings.shortcuts.assist,
        &settings.shortcuts.recording,
        &settings.shortcuts.clear,
    ];
    let parsed = values
        .iter()
        .map(|s| parse(s))
        .collect::<Result<Vec<_>>>()?;
    let old = app
        .try_state::<Arc<Service>>()
        .map(|s| s.snapshot().settings);
    let manager = app.global_shortcut();
    manager
        .unregister_all()
        .map_err(|_| Error::Invalid("Could not update global shortcuts.".into()))?;
    for key in parsed {
        if manager.register(key).is_err() {
            let _ = manager.unregister_all();
            if let Some(old) = old {
                for raw in [
                    &old.shortcuts.visibility,
                    &old.shortcuts.assist,
                    &old.shortcuts.recording,
                    &old.shortcuts.clear,
                ] {
                    if let Ok(key) = parse(raw) {
                        let _ = manager.register(key);
                    }
                }
            }
            return Err(Error::Invalid(
                "A shortcut is already in use or unavailable. Previous bindings were restored."
                    .into(),
            ));
        }
    }
    Ok(())
}
pub fn apply_appearance(app: &tauri::AppHandle, settings: &Settings) -> Result<()> {
    for label in ["main", "overlay"] {
        if let Some(window) = app.get_webview_window(label) {
            window
                .set_content_protected(settings.capture_excluded)
                .map_err(|_| {
                    Error::Invalid("The OS rejected capture exclusion for this window.".into())
                })?;
            if label == "overlay" {
                window.set_focusable(!settings.no_focus).map_err(|_| {
                    Error::Invalid("The OS rejected the overlay focus setting.".into())
                })?
            }
        }
    }
    if let Some(tray) = app.tray_by_id("pluely-tray") {
        let _ = tray.set_visible(!settings.capture_excluded);
    }
    #[cfg(target_os = "macos")]
    {
        let _ = app.set_activation_policy(if settings.capture_excluded {
            tauri::ActivationPolicy::Accessory
        } else {
            tauri::ActivationPolicy::Regular
        });
    }
    if settings.start_at_login {
        app.autolaunch().enable()
    } else {
        app.autolaunch().disable()
    }
    .map_err(|_| Error::Invalid("Could not update launch-at-login preference.".into()))?;
    Ok(())
}
fn show_main(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    }
}
fn action(app: &tauri::AppHandle, name: &str) {
    let service = app.state::<Arc<Service>>().inner().clone();
    let app = app.clone();
    let action = name.to_owned();
    tauri::async_runtime::spawn(async move {
        let result: Result<()> = match action.as_str() {
            "dashboard" => {
                show_main(&app);
                Ok(())
            }
            "visibility" => {
                if let Some(window) = app.get_webview_window("overlay") {
                    if window.is_visible().unwrap_or(false) {
                        let _ = window.hide();
                    } else {
                        let _ = window.show();
                        if !service.snapshot().settings.no_focus {
                            let _ = window.set_focus();
                        }
                    }
                }
                Ok(())
            }
            "record" => {
                if service.snapshot().active_session_id.is_some() {
                    commands::end(service.clone(), app.clone())
                        .await
                        .map(|_| ())
                } else {
                    commands::start(service.clone(), app.clone(), None)
                        .await
                        .map(|_| ())
                }
            }
            "assist" => {
                let id = service.snapshot().active_session_id;
                match id {
                    Some(id) => {
                        let screen = service.snapshot().settings.screen_context;
                        llm::ask(service.clone(),app.clone(),id,"What should I say next? Give a concise, truthful response supported by the current conversation.".into(),screen).await.map(|_|())
                    }
                    None => Err(Error::Invalid(
                        "Start a session before asking about the live conversation.".into(),
                    )),
                }
            }
            "clear" => {
                if let Some(id) = service.snapshot().active_session_id {
                    service.cancel(&format!("chat:{id}"));
                    service
                        .mutate(&app, true, |s| {
                            if let Some(meeting) = s.sessions.iter_mut().find(|s| s.id == id) {
                                meeting.messages.clear()
                            }
                            Ok(())
                        })
                        .map(|_| ())
                } else {
                    Ok(())
                }
            }
            "invisible" => {
                let mut settings = service.snapshot().settings;
                settings.capture_excluded = !settings.capture_excluded;
                apply_appearance(&app, &settings).and_then(|_| {
                    service
                        .mutate(&app, true, |s| {
                            s.settings = settings;
                            Ok(())
                        })
                        .map(|_| ())
                })
            }
            "quit" => {
                if service.snapshot().active_session_id.is_some() {
                    let _ = commands::end(service.clone(), app.clone()).await;
                }
                app.exit(0);
                Ok(())
            }
            _ => Ok(()),
        };
        if let Err(e) = result {
            service.notify(&app, e.to_string());
            show_main(&app);
        }
    });
}
pub fn run() {
    let app=tauri::Builder::default()
 .plugin(tauri_plugin_single_instance::init(|app,_args,_cwd|show_main(app)))
 .plugin(tauri_plugin_dialog::init()).plugin(tauri_plugin_opener::init())
 .plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent,None))
 .plugin(tauri_plugin_global_shortcut::Builder::new().with_handler(|app,shortcut,event|{
  if event.state()!=ShortcutState::Pressed{return}let settings=app.state::<Arc<Service>>().snapshot().settings;
  for (raw,name) in [(&settings.shortcuts.visibility,"visibility"),(&settings.shortcuts.assist,"assist"),(&settings.shortcuts.recording,"record"),(&settings.shortcuts.clear,"clear")]{if parse(raw).is_ok_and(|key|&key==shortcut){action(app,name);break}}
 }).build())
 .setup(|app|{
  let service=Service::open(app.path().app_data_dir()?)?;app.manage(service.clone());
  let overlay=WebviewWindowBuilder::new(app,"overlay",WebviewUrl::App("index.html?window=overlay".into())).title("Pluely assistant").inner_size(690.,500.).min_inner_size(420.,150.).decorations(false).transparent(true).always_on_top(true).skip_taskbar(true).visible(false).resizable(true).build()?;let _=overlay.set_shadow(false);
  let dashboard=MenuItem::with_id(app,"dashboard","Open Pluely",true,None::<&str>)?;
  let record=MenuItem::with_id(app,"record","Start / stop listening",true,None::<&str>)?;
  let visibility=MenuItem::with_id(app,"visibility","Show / hide assistant",true,None::<&str>)?;
  let invisible=MenuItem::with_id(app,"invisible","Toggle capture exclusion",true,None::<&str>)?;
  let quit=MenuItem::with_id(app,"quit","Quit Pluely",true,None::<&str>)?;
  let menu=Menu::with_items(app,&[&dashboard,&record,&visibility,&invisible,&quit])?;
  let icon=tauri::image::Image::from_bytes(include_bytes!("../../public/assets/tray-template@2x.png"))?;
  TrayIconBuilder::with_id("pluely-tray").icon(icon).icon_as_template(true).tooltip("Pluely").menu(&menu).show_menu_on_left_click(true).on_menu_event(|app,event|action(app,event.id.as_ref())).build(app)?;
  let settings=service.snapshot().settings;register_shortcuts(app.handle(),&settings)?;if let Err(e)=apply_appearance(app.handle(),&settings){service.notify(app.handle(),e.to_string());}
  if let Some(main)=app.get_webview_window("main"){let w=main.clone();main.on_window_event(move|event|{if let tauri::WindowEvent::CloseRequested{api,..}=event{api.prevent_close();let _=w.hide();}});}
  let handle=app.handle().clone();tauri::async_runtime::spawn(async move{loop{tokio::time::sleep(std::time::Duration::from_secs(30)).await;let snapshot=service.snapshot();let recording=snapshot.active_session_id.as_ref().is_some_and(|id|snapshot.sessions.iter().any(|s|&s.id==id&&s.status=="recording"));let idle=service.last_activity.lock().unwrap().elapsed().as_secs()>600;if recording&&idle{let _=commands::end(service.clone(),handle.clone()).await;service.notify(&handle,"The session ended after ten minutes without speech or assistant activity.");}}});
  Ok(())
 })
 .on_page_load(|window,_|{let _=window.emit("pluely:ready",true);})
 .invoke_handler(tauri::generate_handler![commands::get_snapshot,commands::get_storage_status,commands::get_capabilities,commands::provider_status,commands::configure_storage,commands::unlock_history,commands::save_credential,commands::delete_credential,commands::save_settings,commands::save_mode,commands::select_mode,commands::delete_mode,commands::update_session,commands::delete_session,commands::start_session,commands::resume_session,commands::pause_capture,commands::resume_capture,commands::end_session,commands::ask,commands::generate_notes,commands::cancel_response,commands::clear_conversation,commands::show_overlay,commands::hide_overlay,commands::test_llm,commands::list_local_models,commands::download_model,commands::cancel_model_download,commands::import_model,commands::import_context_file,commands::export_session,commands::export_workspace,commands::import_workspace,commands::open_data_folder])
 .build(tauri::generate_context!()).expect("Pluely could not initialize its desktop runtime");
    app.run(|app,event|{match event{tauri::RunEvent::ExitRequested{api,code:None,..}=>{let service=app.state::<Arc<Service>>();if service.snapshot().active_session_id.is_some(){api.prevent_exit();let app=app.clone();tauri::async_runtime::spawn(async move{use tauri_plugin_dialog::DialogExt;let a=app.clone();let confirmed=tauri::async_runtime::spawn_blocking(move||a.dialog().message("A meeting is still recording. Quitting stops capture; pending transcription may not finish. Use End Session to flush and save all pending speech first.").title("Quit Pluely?").buttons(tauri_plugin_dialog::MessageDialogButtons::OkCancel).blocking_show()).await.unwrap_or(false);if confirmed{action(&app,"quit");}});}},#[cfg(target_os="macos")]tauri::RunEvent::Reopen{..}=>show_main(app),_=>{}}});
}
