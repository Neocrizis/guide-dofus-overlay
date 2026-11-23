// Guide Dofus Overlay : affiche le journal du Guide Dofus (site en ligne, ?mode=overlay)
// dans une fenêtre sans cadre, transparente et toujours au-dessus du jeu.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::webview::PageLoadEvent;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder, Wry};
use tauri_plugin_deep_link::DeepLinkExt;
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

const SITE: &str = "https://neocrizis.github.io/guide-dofus/?mode=overlay";
const HOTE: &str = "neocrizis.github.io";

/// Clic au travers : la souris passe à travers la fenêtre pour jouer dessous.
static TRAVERS: AtomicBool = AtomicBool::new(false);
static DESSUS: AtomicBool = AtomicBool::new(true);

struct Menus {
    travers: CheckMenuItem<Wry>,
    dessus: CheckMenuItem<Wry>,
}
/// Lien de connexion reçu avant que la page soit prête.
struct LienEnAttente(Mutex<Option<String>>);

fn journal(app: &AppHandle) -> Option<WebviewWindow> {
    app.get_webview_window("journal")
}

fn montrer(app: &AppHandle) {
    if let Some(w) = journal(app) {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

fn basculer_visible(app: &AppHandle) {
    if let Some(w) = journal(app) {
        if w.is_visible().unwrap_or(false) {
            let _ = w.hide();
        } else {
            montrer(app);
        }
    }
}

fn regler_travers(app: &AppHandle, actif: bool) {
    TRAVERS.store(actif, Ordering::SeqCst);
    if let Some(w) = journal(app) {
        let _ = w.set_ignore_cursor_events(actif);
        let _ = w.eval(&format!(
            "document.documentElement.classList.toggle('ov-travers', {actif})"
        ));
    }
    if let Some(m) = app.try_state::<Menus>() {
        let _ = m.travers.set_checked(actif);
    }
}

fn regler_dessus(app: &AppHandle, actif: bool) {
    DESSUS.store(actif, Ordering::SeqCst);
    if let Some(w) = journal(app) {
        let _ = w.set_always_on_top(actif);
    }
    if let Some(m) = app.try_state::<Menus>() {
        let _ = m.dessus.set_checked(actif);
    }
}

/// Transmet un lien guidedofus://auth?code=… à la page, qui termine la connexion au compte.
fn transmettre_lien(app: &AppHandle, lien: &str) {
    if !lien.starts_with("guidedofus://") {
        return;
    }
    montrer(app);
    match journal(app) {
        Some(w) => {
            let texte = serde_json::to_string(lien).unwrap_or_default();
            let _ = w.eval(&format!(
                "window.GuideDofusAuth ? window.GuideDofusAuth({texte}) : (window.__lienAuth = {texte})"
            ));
        }
        None => {
            if let Some(etat) = app.try_state::<LienEnAttente>() {
                *etat.0.lock().unwrap() = Some(lien.to_string());
            }
        }
    }
}

fn main() {
    let raccourci_visible = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::KeyG);
    let raccourci_travers = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::KeyT);

    tauri::Builder::default()
        // Une seule instance : un lien guidedofus:// ouvert depuis le navigateur arrive dans l'appli déjà lancée.
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            match args.iter().find(|a| a.starts_with("guidedofus://")) {
                Some(lien) => transmettre_lien(app, lien),
                None => montrer(app),
            }
        }))
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(move |app, raccourci, evenement| {
                    if evenement.state() != ShortcutState::Pressed {
                        return;
                    }
                    if raccourci == &raccourci_visible {
                        basculer_visible(app);
                    } else if raccourci == &raccourci_travers {
                        regler_travers(app, !TRAVERS.load(Ordering::SeqCst));
                    }
                })
                .build(),
        )
        .manage(LienEnAttente(Mutex::new(None)))
        .setup(move |app| {
            let handle = app.handle().clone();

            // Lien de connexion qui a lancé l'appli (si elle n'était pas ouverte).
            #[cfg(any(windows, target_os = "linux"))]
            {
                let _ = app.deep_link().register_all();
            }
            if let Some(lien) = std::env::args().find(|a| a.starts_with("guidedofus://")) {
                *app.state::<LienEnAttente>().0.lock().unwrap() = Some(lien);
            }
            let h = handle.clone();
            app.deep_link().on_open_url(move |ev| {
                for url in ev.urls() {
                    transmettre_lien(&h, url.as_str());
                }
            });

            let h = handle.clone();
            WebviewWindowBuilder::new(app, "journal", WebviewUrl::External(SITE.parse().unwrap()))
                .title("Guide Dofus")
                .inner_size(400.0, 640.0)
                .min_inner_size(280.0, 260.0)
                .decorations(false)
                .transparent(true)
                .shadow(false)
                .always_on_top(true)
                .resizable(true)
                // Les liens externes (DofusDB, connexion Google…) s'ouvrent dans le navigateur du PC.
                .on_navigation(|url| {
                    let interne = url.scheme() == "https" && url.host_str() == Some(HOTE);
                    if !interne && (url.scheme() == "https" || url.scheme() == "http") {
                        let _ = open::that_detached(url.as_str());
                    }
                    interne
                })
                .on_page_load(move |w, charge| {
                    if charge.event() != PageLoadEvent::Finished {
                        return;
                    }
                    let travers = TRAVERS.load(Ordering::SeqCst);
                    let _ = w.eval(&format!(
                        "document.documentElement.classList.add('appli'); document.documentElement.classList.toggle('ov-travers', {travers})"
                    ));
                    let lien = h.state::<LienEnAttente>().0.lock().unwrap().take();
                    if let Some(lien) = lien {
                        let h2 = h.clone();
                        // Laisse le temps au script du compte de démarrer.
                        std::thread::spawn(move || {
                            std::thread::sleep(std::time::Duration::from_secs(2));
                            transmettre_lien(&h2, &lien);
                        });
                    }
                })
                .build()?;

            // Icône dans la zone de notification, avec son menu.
            let afficher = MenuItem::with_id(app, "afficher", "Afficher / masquer    Ctrl+Maj+G", true, None::<&str>)?;
            let travers = CheckMenuItem::with_id(app, "travers", "Clic au travers    Ctrl+Maj+T", true, false, None::<&str>)?;
            let dessus = CheckMenuItem::with_id(app, "dessus", "Toujours au premier plan", true, true, None::<&str>)?;
            let recharger = MenuItem::with_id(app, "recharger", "Recharger le guide", true, None::<&str>)?;
            let quitter = MenuItem::with_id(app, "quitter", "Quitter", true, None::<&str>)?;
            let sep = PredefinedMenuItem::separator(app)?;
            let menu = Menu::with_items(app, &[&afficher, &travers, &dessus, &recharger, &sep, &quitter])?;
            app.manage(Menus { travers: travers.clone(), dessus: dessus.clone() });

            TrayIconBuilder::with_id("guide")
                .icon(app.default_window_icon().unwrap().clone())
                .tooltip("Guide Dofus : Ctrl+Maj+G pour afficher, Ctrl+Maj+T pour le clic au travers")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, ev| match ev.id().as_ref() {
                    "afficher" => basculer_visible(app),
                    "travers" => regler_travers(app, !TRAVERS.load(Ordering::SeqCst)),
                    "dessus" => regler_dessus(app, !DESSUS.load(Ordering::SeqCst)),
                    "recharger" => {
                        if let Some(w) = journal(app) {
                            let _ = w.eval("location.reload()");
                        }
                    }
                    "quitter" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, ev| {
                    if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = ev {
                        basculer_visible(tray.app_handle());
                    }
                })
                .build(app)?;

            app.global_shortcut().register(raccourci_visible)?;
            app.global_shortcut().register(raccourci_travers)?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("impossible de lancer Guide Dofus Overlay");
}
